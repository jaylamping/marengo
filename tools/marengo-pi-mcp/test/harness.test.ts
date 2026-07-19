import { describe, it } from "node:test";
import assert from "node:assert/strict";
import type { MarengoPiConfig } from "../src/config.js";
import { harnessConfigDir } from "../src/harness/index.js";

const cfg: MarengoPiConfig = {
  host: "marengo.local",
  user: "joey",
  piRoot: "/opt/marengo",
  configDir: "/opt/marengo/config/bringup/shoulder_pitch_dual",
  localRoot: "/tmp/marengo",
  benchProfile: "bare_motor",
  piStagingRoot: "~/marengo",
};

describe("bench harness config", () => {
  it("selects shoulder_pitch_weighted for weighted_single_arm profile", () => {
    assert.equal(
      harnessConfigDir(cfg, "weighted_single_arm"),
      "/opt/marengo/config/bringup/shoulder_pitch_weighted",
    );
  });

  it("keeps default config for bare_motor profile", () => {
    assert.equal(harnessConfigDir(cfg, "bare_motor"), cfg.configDir);
  });

  it("expands relative config_dir overrides", () => {
    assert.equal(
      harnessConfigDir(cfg, "bare_motor", "arm_2dof_right"),
      "/opt/marengo/config/bringup/arm_2dof_right",
    );
  });

  it("selects arm_2dof_right for roll_attached profile", () => {
    assert.equal(
      harnessConfigDir(cfg, "roll_attached"),
      "/opt/marengo/config/bringup/arm_2dof_right",
    );
  });

  it("selects arm_2dof_right for arm_2dof_smoke profile", () => {
    assert.equal(
      harnessConfigDir(cfg, "arm_2dof_smoke"),
      "/opt/marengo/config/bringup/arm_2dof_right",
    );
  });
});
