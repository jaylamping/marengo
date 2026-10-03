import { describe, it } from "node:test";
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import {
  BENCH_PROFILES,
  BENCH_PROFILE_META,
  MASTER_JOINTS,
  harnessJointSubset,
  weightedProfiles,
  type BenchProfile,
} from "../src/bench-profiles.js";
import { WEIGHTED_PROFILES } from "../src/safety.js";
import { harnessConfigDir } from "../src/harness/index.js";
import type { MarengoPiConfig } from "../src/config.js";

const REPO_CONFIG = new URL("../../../config/", import.meta.url);

/** `robot.joints` list of the repo master config/robot.yaml, in order. */
function robotYamlJoints(): string[] {
  const lines = readFileSync(new URL("robot.yaml", REPO_CONFIG), "utf8").split("\n");
  const start = lines.findIndex((l) => /^ {2}joints:\s*$/.test(l));
  assert.ok(start >= 0, "config/robot.yaml has robot.joints");
  const joints: string[] = [];
  for (const line of lines.slice(start + 1)) {
    const m = /^ {4}- (\S+)\s*$/.exec(line);
    if (!m) break;
    joints.push(m[1]);
  }
  return joints;
}

const cfg: MarengoPiConfig = {
  host: "marengo.local",
  user: "joey",
  piRoot: "/opt/marengo",
  configDir: "/opt/marengo/config",
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
      for (const joint of meta.setZeroJoints) {
        assert.equal(meta.hangingRestRad[joint], 0, `${profile} ${joint} hangs at its reference`);
      }
    }
  });

  it("keeps WEIGHTED_PROFILES in sync with metadata", () => {
    assert.deepEqual(WEIGHTED_PROFILES, weightedProfiles());
  });

  it("uses master config dir for right-arm profiles", () => {
    const right3: BenchProfile[] = ["roll_attached", "arm_2dof_smoke"];
    for (const p of right3) {
      assert.equal(harnessConfigDir(cfg, p), "/opt/marengo/config");
      assert.equal(
        harnessJointSubset(p),
        "right_shoulder_pitch,right_shoulder_roll,right_upper_arm_yaw",
      );
    }
    for (const p of ["yaw_attached", "elbow_attached"] as BenchProfile[]) {
      assert.equal(harnessConfigDir(cfg, p), "/opt/marengo/config");
      assert.ok(harnessJointSubset(p)?.includes("right_elbow_pitch"));
    }
  });

  it("maps 3-DOF smoke to MARENGO_JOINT_SUBSET", () => {
    assert.equal(
      harnessJointSubset("arm_2dof_smoke"),
      "right_shoulder_pitch,right_shoulder_roll,right_upper_arm_yaw",
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

describe("bench profiles match the live master config", () => {
  const robotJoints = robotYamlJoints();

  it("MASTER_JOINTS is config/robot.yaml robot.joints and motors.yaml, in order", () => {
    assert.deepEqual([...MASTER_JOINTS], robotJoints);
    const motorsYaml = readFileSync(new URL("motors.yaml", REPO_CONFIG), "utf8");
    const motorJoints = [...motorsYaml.matchAll(/^ {2}- joint: (\S+)\s*$/gm)].map((m) => m[1]);
    assert.deepEqual(motorJoints, robotJoints);
  });

  it("full-master profiles reference (and gravity-gate) exactly the robot.yaml joints", () => {
    for (const profile of BENCH_PROFILES) {
      const meta = BENCH_PROFILE_META[profile];
      if (meta.jointSubset) continue;
      assert.deepEqual(meta.setZeroJoints, robotJoints, profile);
      assert.deepEqual(Object.keys(meta.hangingRestRad), robotJoints, profile);
    }
    assert.deepEqual(BENCH_PROFILE_META.arm_attached.setZeroJoints, [
      "right_shoulder_pitch",
      "right_shoulder_roll",
      "right_upper_arm_yaw",
      "right_elbow_pitch",
      "right_lower_arm_yaw",
    ]);
  });

  it("subset profiles are robot.yaml-order prefixes and reference their whole subset", () => {
    for (const profile of BENCH_PROFILES) {
      const subset = BENCH_PROFILE_META[profile].jointSubset;
      if (!subset) continue;
      assert.deepEqual([...subset], robotJoints.slice(0, subset.length), profile);
      assert.deepEqual(BENCH_PROFILE_META[profile].setZeroJoints, [...subset], profile);
    }
  });
});
