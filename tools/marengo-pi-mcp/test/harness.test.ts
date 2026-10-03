import { describe, it } from "node:test";
import assert from "node:assert/strict";
import type { MarengoPiConfig } from "../src/config.js";
import { harnessConfigDir, runBenchHarness } from "../src/harness/index.js";
import { harnessJointSubset } from "../src/bench-profiles.js";
import { wrapRemoteWithConfig } from "../src/env.js";

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
        return body.includes("homing-preflight.sh") ? "homing=Verified" : "ok";
      },
      { profile: "arm_2dof_smoke", skip_set_zero: true },
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
        return body.includes("homing-preflight.sh") ? "homing=Verified" : "ok";
      },
      { profile: "arm_2dof_smoke", skip_set_zero: true },
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
      { profile: "arm_2dof_smoke", skip_set_zero: true },
    );

    assert.ok(!bodies.some((b) => b.includes("can-up.sh") || b.includes("bin/motor-repl")));
    assert.match(bodies[bodies.length - 1], /pi-restart-marengo-pi\.sh' restart/);
  });

  it("clears a sourced joint subset when no override is supplied", () => {
    const wrapped = wrapRemoteWithConfig(cfg, "true", cfg.configDir);
    assert.match(wrapped, /unset MARENGO_JOINT_SUBSET/);
    assert.doesNotMatch(wrapped, /export MARENGO_JOINT_SUBSET=/);
  });
});
