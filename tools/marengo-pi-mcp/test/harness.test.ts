import { describe, it } from "node:test";
import assert from "node:assert/strict";
import type { MarengoPiConfig } from "../src/config.js";
import { harnessConfigDir, runBenchHarness } from "../src/harness/index.js";
import { harnessJointSubset } from "../src/bench-profiles.js";
import { wrapRemoteWithConfig } from "../src/env.js";
import { harnessScriptSuite } from "../src/harness/scripts.js";
import { gravityPreviewReply, isGravityPreviewBody } from "./gravity-fixture.js";

const OPT_IN = { set_zero: true, at_mechanical_reference: true } as const;

const cfg: MarengoPiConfig = {
  host: "marengo.local",
  user: "joey",
  piRoot: "/opt/marengo",
  configDir: "/opt/marengo/config",
  localRoot: "/tmp/marengo",
  benchProfile: "bare_motor",
  piStagingRoot: "~/marengo",
};

describe("bench harness config", () => {
  it("uses master config for weighted_single_arm profile", () => {
    assert.equal(
      harnessConfigDir(cfg, "weighted_single_arm"),
      "/opt/marengo/config",
    );
  });

  it("keeps default master config for bare_motor profile", () => {
    assert.equal(harnessConfigDir(cfg, "bare_motor"), cfg.configDir);
  });

  it("ignores legacy bringup slug overrides", () => {
    assert.equal(
      harnessConfigDir(cfg, "bare_motor", "arm_3dof_right"),
      "/opt/marengo/config",
    );
  });

  it("accepts absolute config_dir overrides", () => {
    assert.equal(
      harnessConfigDir(cfg, "bare_motor", "/tmp/custom-config"),
      "/tmp/custom-config",
    );
  });

  it("exports joint subset for roll_attached", () => {
    assert.equal(
      harnessJointSubset("roll_attached"),
      "right_shoulder_pitch,right_shoulder_roll,right_upper_arm_yaw",
    );
  });

  it("exports the profile joint subset in scripted harness sessions", async () => {
    const bodies: string[] = [];
    await runBenchHarness(
      cfg,
      async (body) => {
        bodies.push(body);
        return isGravityPreviewBody(body) ? gravityPreviewReply() : "ok";
      },
      { profile: "arm_2dof_smoke", ...OPT_IN },
    );

    const scripted = bodies.find((body) => body.includes("=== bench harness"));
    assert.ok(scripted);
    assert.match(
      scripted,
      /export MARENGO_JOINT_SUBSET='right_shoulder_pitch,right_shoulder_roll,right_upper_arm_yaw'/,
    );
  });

  it("takes CAN before can_up and restores an active unit after the run", async () => {
    const bodies: string[] = [];
    const out = await runBenchHarness(
      cfg,
      async (body) => {
        bodies.push(body);
        if (body.includes("restore after session")) {
          return "marengo-pi.service restore after session: true";
        }
        return "ok";
      },
      { profile: "arm_2dof_smoke", ...OPT_IN },
    );

    const take = bodies.findIndex((b) => b.includes("pi-restart-marengo-pi.sh' stop"));
    const canUp = bodies.findIndex((b) => b.includes("can-up.sh"));
    const firstRepl = bodies.findIndex((b) => b.includes("bin/motor-repl"));
    assert.ok(take >= 0 && take < canUp && take < firstRepl);
    assert.match(bodies[bodies.length - 1], /MARENGO_PI_UNIT_RESTORE=true\n[\s\S]*pi-restart-marengo-pi\.sh' restart/);
    assert.match(out, /\[PASS\] restore_marengo_pi_service/);
  });

  it("stops after a refused CAN take and still restores the unit", async () => {
    const bodies: string[] = [];
    await runBenchHarness(
      cfg,
      async (body) => {
        bodies.push(body);
        return body.includes("restore after session")
          ? "marengo-pi.service restore after session: true\n[exit 1]"
          : "ok";
      },
      { profile: "arm_2dof_smoke", ...OPT_IN },
    );

    assert.ok(!bodies.some((b) => b.includes("can-up.sh") || b.includes("bin/motor-repl")));
    assert.match(bodies[bodies.length - 1], /pi-restart-marengo-pi\.sh' restart/);
  });

  it("refuses enable-requiring suites without the reference opt-in before touching the Pi", async () => {
    let calls = 0;
    const out = await runBenchHarness(
      cfg,
      async (body) => {
        calls += 1;
        return body;
      },
      { profile: "arm_2dof_smoke", set_zero: true },
    );

    assert.equal(calls, 0);
    assert.match(out, /\[FAIL\] reference_required/);
    assert.match(out, /at_mechanical_reference: true/);
  });

  it("runs every suite in one marengo-pi session after a single awaited acquisition", async () => {
    const names = harnessScriptSuite("arm_2dof_smoke")?.scripts.map((s) => s.name) ?? [];
    assert.ok(names.length >= 2);
    const bodies: string[] = [];
    const out = await runBenchHarness(
      cfg,
      async (body) => {
        bodies.push(body);
        if (isGravityPreviewBody(body)) return gravityPreviewReply();
        if (!body.includes("sign-tested")) return "ok";
        return [
          "reference right_shoulder_pitch current pos=0.0000",
          "reference right_shoulder_roll current pos=0.0001",
          "reference right_upper_arm_yaw current pos=-0.0001",
          `=== harness suite ${names[0]} ===`,
          "operational: Active",
          `=== harness suite ${names[1]} ===`,
          "enable blocked: Enable requires full-master Robot Ready",
          "harness: earlier suite failed; not starting next",
          "[exit 1]",
        ].join("\n");
      },
      { profile: "arm_2dof_smoke", ...OPT_IN },
    );

    const sessions = bodies.filter((b) => b.includes("sign-tested"));
    assert.equal(sessions.length, 1);
    const session = sessions[0];
    assert.match(session, /"home right_shoulder_pitch right_shoulder_roll right_upper_arm_yaw sign-tested"/);
    for (const name of names) assert.match(session, new RegExp(`=== harness suite ${name} ===`));
    assert.equal(session.match(/printf '%s\\n' "quit"/g)?.length, 1);
    // The bus settles (no CAN owner, error counters still) before marengo-pi binds can0.
    assert.match(session, /can_error_counters\(\) \{[\s\S]*can settle: ok[\s\S]*\} \| timeout \d+ bin\/marengo-pi/);
    assert.ok(!bodies.some((b) => b.includes("motor-repl set-zero") || b.includes("motor-repl homing-status")));

    assert.match(out, /\[PASS\] reference_acquire/);
    assert.match(out, new RegExp(`\\[PASS\\] ${names[0]}\\n`));
    assert.match(out, new RegExp(`\\[FAIL\\] ${names[1]}\\n`));
    for (const name of names.slice(2)) assert.match(out, new RegExp(`\\[FAIL\\] ${name}\\n`));
  });

  it("refuses on a gravity model mismatch before any enable, then restores the unit", async () => {
    const bodies: string[] = [];
    const out = await runBenchHarness(
      cfg,
      async (body) => {
        bodies.push(body);
        if (body.includes("restore after session")) {
          return "marengo-pi.service restore after session: true";
        }
        // Live-URDF regression from the ascent-stall diagnosis: hanging arm, model says -1.6 Nm.
        return isGravityPreviewBody(body)
          ? gravityPreviewReply({ right_shoulder_pitch: -1.5951 })
          : "ok";
      },
      { profile: "arm_2dof_smoke", ...OPT_IN },
    );

    const take = bodies.findIndex((b) => b.includes("pi-restart-marengo-pi.sh' stop"));
    const preview = bodies.findIndex(isGravityPreviewBody);
    assert.ok(take >= 0 && take < preview);
    assert.ok(
      !bodies.some((b) => /sign-tested|enable bench|hold-on|gravity-on|=== bench harness/.test(b)),
      "no motion session may start after a gravity_model_mismatch",
    );
    assert.match(out, /\[FAIL\] gravity_gate/);
    assert.match(out, /FAIL gravity_model_mismatch: \|τ_g\| at the hanging rest ≥ 0\.20 Nm on right_shoulder_pitch/);
    assert.match(out, /\[PASS\] restore_marengo_pi_service/);
  });

  it("runs the gravity gate for every profile, including right-arm ones", async () => {
    for (const profile of ["bare_motor", "roll_attached", "yaw_attached", "elbow_attached"] as const) {
      const bodies: string[] = [];
      const out = await runBenchHarness(
        cfg,
        async (body) => {
          bodies.push(body);
          return isGravityPreviewBody(body) ? gravityPreviewReply() : "ok";
        },
        { profile, ...OPT_IN },
      );
      assert.ok(bodies.some(isGravityPreviewBody), profile);
      assert.match(out, /\[PASS\] gravity_gate/, profile);
    }
  });

  it("clears a sourced joint subset when no override is supplied", () => {
    const wrapped = wrapRemoteWithConfig(cfg, "true", cfg.configDir);
    assert.match(wrapped, /unset MARENGO_JOINT_SUBSET/);
    assert.doesNotMatch(wrapped, /export MARENGO_JOINT_SUBSET=/);
  });
});
