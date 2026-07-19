import { describe, it } from "node:test";
import assert from "node:assert/strict";
import { BENCH_PROFILES } from "../src/bench-profiles.js";
import { harnessScriptSuite } from "../src/harness/scripts.js";

describe("harnessScriptSuite", () => {
  it("yaw_attached is smoke with operator sign-off required", () => {
    const suite = harnessScriptSuite("yaw_attached");
    assert.ok(suite);
    assert.equal(suite.passKind, "smoke");
    assert.equal(suite.operatorSignoffRequired, true);
    assert.equal(suite.note?.name, "yaw_suite_smoke_note");
    assert.ok(suite.scripts.some((s) => s.name === "yaw_sign_probe"));
    assert.ok(suite.scripts.some((s) => s.name === "yaw_hold_ladder"));
    assert.ok(suite.scripts.some((s) => s.name === "yaw_cross_talk"));
  });

  it("scripted profiles share data-driven suite (no orphan copy-paste)", () => {
    for (const profile of [
      "roll_attached",
      "arm_2dof_smoke",
      "yaw_attached",
      "bare_motor",
    ] as const) {
      const suite = harnessScriptSuite(profile);
      assert.ok(suite, `missing suite for ${profile}`);
      assert.ok(suite.scripts.length >= 1);
      assert.equal(suite.passKind, "smoke");
    }
  });

  it("weighted profiles have no pipe script suite", () => {
    assert.equal(harnessScriptSuite("weighted_single_arm"), null);
    assert.equal(harnessScriptSuite("arm_attached"), null);
  });

  it("every BenchProfile is either scripted or intentionally null", () => {
    for (const profile of BENCH_PROFILES) {
      const suite = harnessScriptSuite(profile);
      if (profile === "weighted_single_arm" || profile === "arm_attached") {
        assert.equal(suite, null);
      } else {
        assert.ok(suite, `expected suite for ${profile}`);
      }
    }
  });
});
