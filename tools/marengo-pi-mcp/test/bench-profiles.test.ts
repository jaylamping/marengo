import { describe, it } from "node:test";
import assert from "node:assert/strict";
import {
  BENCH_PROFILES,
  BENCH_PROFILE_META,
  isRightArmBenchProfile,
  weightedProfiles,
  type BenchProfile,
} from "../src/bench-profiles.js";
import { WEIGHTED_PROFILES } from "../src/safety.js";
import { harnessConfigDir } from "../src/harness/index.js";
import type { MarengoPiConfig } from "../src/config.js";

const cfg: MarengoPiConfig = {
  host: "marengo.local",
  user: "joey",
  piRoot: "/opt/marengo",
  configDir: "/opt/marengo/config/bringup/shoulder_pitch_dual",
  localRoot: "/tmp/marengo",
  benchProfile: "bare_motor",
  piStagingRoot: "~/marengo",
};

describe("bench profile metadata", () => {
  it("has exhaustive meta for every BenchProfile", () => {
    for (const profile of BENCH_PROFILES) {
      const meta = BENCH_PROFILE_META[profile];
      assert.ok(meta, `missing meta for ${profile}`);
      assert.ok(Array.isArray(meta.setZeroJoints));
      assert.ok(meta.setZeroJoints.length > 0);
      assert.equal(typeof meta.weighted, "boolean");
      assert.equal(typeof meta.skipGravityPreview, "boolean");
    }
  });

  it("keeps WEIGHTED_PROFILES in sync with metadata", () => {
    assert.deepEqual(WEIGHTED_PROFILES, weightedProfiles());
  });

  it("marks right-arm bringup profiles via configBringup", () => {
    const right3: BenchProfile[] = ["roll_attached", "arm_2dof_smoke"];
    for (const p of right3) {
      assert.equal(isRightArmBenchProfile(p), true);
      assert.equal(
        harnessConfigDir(cfg, p),
        "/opt/marengo/config/bringup/arm_3dof_right",
      );
    }
    for (const p of ["yaw_attached", "elbow_attached"] as BenchProfile[]) {
      assert.equal(isRightArmBenchProfile(p), true);
      assert.equal(
        harnessConfigDir(cfg, p),
        "/opt/marengo/config/bringup/arm_4dof_right",
      );
    }
  });

  it("selects arm_4dof_right for yaw_attached and elbow_attached", () => {
    assert.equal(
      harnessConfigDir(cfg, "yaw_attached"),
      "/opt/marengo/config/bringup/arm_4dof_right",
    );
    assert.equal(
      harnessConfigDir(cfg, "elbow_attached"),
      "/opt/marengo/config/bringup/arm_4dof_right",
    );
  });

  it("includes yaw and elbow in 4-DOF set-zero joints", () => {
    for (const profile of ["yaw_attached", "elbow_attached"] as const) {
      const joints = BENCH_PROFILE_META[profile].setZeroJoints;
      assert.ok(joints.includes("right_upper_arm_yaw"));
      assert.ok(joints.includes("right_elbow_pitch"));
      assert.ok(joints.includes("right_shoulder_pitch"));
      assert.ok(joints.includes("right_shoulder_roll"));
    }
  });
});
