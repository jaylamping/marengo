import { describe, it } from "node:test";
import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { chmodSync, mkdirSync, mkdtempSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import path from "node:path";
import { gravityGateSnapshotShell, runGravityGate } from "../src/gravity-gate.js";
import { encodeRobotState, type JointFixture } from "./robot-state-fixture.js";
import { gravityPreviewReply } from "./gravity-fixture.js";

const NOW_MS = 1_790_000_000_000;

function snapshotOutput(ageMs: number, joints: JointFixture[]): string {
  const b64 = Buffer.from(encodeRobotState(NOW_MS - ageMs, joints)).toString("base64");
  return `pi_now_ms=${NOW_MS}\nrobot_state_b64=${b64}\n`;
}

/**
 * Run a preview shell against a fake motor-repl whose model is three joints in robot.yaml
 * order and whose τ_g echoes each joint's angle (0 when the vector is incomplete, like the CLI).
 */
function runWithFakeMotorRepl(shell: string): string {
  const dir = mkdtempSync(path.join(tmpdir(), "gravity-gate-"));
  mkdirSync(path.join(dir, "bin"));
  writeFileSync(
    path.join(dir, "bin", "motor-repl"),
    [
      "#!/bin/bash",
      "shift",
      "names=(right_shoulder_pitch right_shoulder_roll right_upper_arm_yaw)",
      'if [[ $# -ne ${#names[@]} ]]; then set -- 0 0 0; fi',
      'for n in "${names[@]}"; do echo "$n: tau_g = $1 Nm"; shift; done',
    ].join("\n"),
  );
  chmodSync(path.join(dir, "bin", "motor-repl"), 0o755);
  const r = spawnSync("bash", ["-c", `set -euo pipefail\n${shell}`], { cwd: dir, encoding: "utf8" });
  assert.equal(r.status, 0, r.stderr);
  return r.stdout;
}

describe("gravity model gate", () => {
  it("reads the Pi clock and gateway snapshot without motor-repl", () => {
    const shell = gravityGateSnapshotShell();
    assert.match(shell, /pi_now_ms=%s\\n' "\$\(date \+%s%3N\)"/);
    assert.match(shell, /snapshot\/robot\/state/);
    assert.doesNotMatch(shell, /motor-repl/);
  });

  it("falls back to the hanging rest pose while drives are disabled", async () => {
    const shells: string[] = [];
    const gate = await runGravityGate({
      profile: "roll_attached",
      joints: ["right_shoulder_pitch", "right_shoulder_roll", "right_upper_arm_yaw"],
      snapshotOutput: snapshotOutput(20, [
        { name: "right_shoulder_pitch", homing: 1, driveActive: false, effort: 0 },
      ]),
      runPreview: async (shell) => {
        shells.push(shell);
        return gravityPreviewReply({ right_shoulder_pitch: -0.19 });
      },
    });
    assert.deepEqual(shells, ["bin/motor-repl gravity-preview"]);
    assert.equal(gate.ok, true);
    assert.match(gate.report, /basis=hanging_rest\): not drive-active\+Verified: right_shoulder_pitch, right_shoulder_roll, right_upper_arm_yaw/);
  });

  it("treats a stale or missing snapshot as no live torque", async () => {
    for (const out of [snapshotOutput(5_000, []), "robot_state_b64=\n", ""]) {
      const gate = await runGravityGate({
        profile: "elbow_attached",
        joints: ["right_shoulder_pitch", "right_elbow_pitch"],
        snapshotOutput: out,
        runPreview: async () => gravityPreviewReply({ right_elbow_pitch: 0.2 }),
      });
      assert.equal(gate.ok, false);
      assert.match(gate.report, /basis=hanging_rest/);
      assert.match(gate.report, /FAIL gravity_model_mismatch: \|τ_g\| at the hanging rest ≥ 0\.20 Nm on right_elbow_pitch\./);
    }
  });

  it("refuses when the preview yields no τ_g for any gated joint", async () => {
    const gate = await runGravityGate({
      profile: "bare_motor",
      joints: ["left_shoulder_pitch"],
      snapshotOutput: "",
      runPreview: async () => "error: right_shoulder_pitch (pid 4242) still owns CAN\n[exit 1]",
    });
    assert.equal(gate.ok, false);
    assert.match(gate.report, /left_shoulder_pitch: q=0\.0000 rad — not in the gravity model/);
    assert.match(gate.report, /FAIL gravity_gate_unavailable: gravity-preview returned no τ_g for left_shoulder_pitch/);
    assert.match(gate.report, /^error: .*still owns CAN/);
  });

  it("evaluates the published pose as a full robot.yaml-order vector", async () => {
    const gate = await runGravityGate({
      profile: "arm_2dof_smoke",
      joints: ["right_shoulder_pitch", "right_upper_arm_yaw"],
      snapshotOutput: snapshotOutput(10, [
        // Snapshot order differs from the model order on purpose.
        { name: "right_upper_arm_yaw", homing: 3, driveActive: true, position: 0.3, effort: 0.25 },
        { name: "right_shoulder_pitch", homing: 3, driveActive: true, position: 0, effort: 0.1 },
      ]),
      runPreview: async (shell) => runWithFakeMotorRepl(shell),
    });
    assert.match(gate.report, /right_shoulder_pitch: q=0\.0000 rad τ_g=0\.0000 Nm τ_meas=0\.1000 Nm residual=0\.1000 Nm ok/);
    assert.match(gate.report, /right_upper_arm_yaw: q=0\.3000 rad τ_g=0\.3000 Nm τ_meas=0\.2500 Nm residual=0\.0500 Nm ok/);
    assert.equal(gate.ok, true);
  });

  it("fails at exactly the 0.20 Nm bar", async () => {
    const gate = await runGravityGate({
      profile: "roll_attached",
      joints: ["right_shoulder_roll"],
      snapshotOutput: "",
      runPreview: async () => gravityPreviewReply({ right_shoulder_roll: -0.2 }),
    });
    assert.equal(gate.ok, false);
  });
});
