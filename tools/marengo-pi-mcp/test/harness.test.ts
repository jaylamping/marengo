import { describe, it } from "node:test";
import assert from "node:assert/strict";
import type { MarengoPiConfig } from "../src/config.js";
import { harnessConfigDir } from "../src/harness/index.js";
import { harnessJointSubset } from "../src/bench-profiles.js";

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
      "right_shoulder_roll,right_shoulder_pitch,right_upper_arm_yaw",
    );
  });
});
