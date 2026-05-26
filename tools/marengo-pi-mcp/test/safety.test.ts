import { describe, it } from "node:test";
import assert from "node:assert/strict";
import {
  requiresWeightedDoubleConfirm,
  validateMotionConfirm,
} from "../src/safety.js";

describe("validateMotionConfirm", () => {
  it("blocks motion without confirm", () => {
    const r = validateMotionConfirm({}, "bare_motor");
    assert.equal(r.ok, false);
  });

  it("allows bare_motor with confirm only", () => {
    const r = validateMotionConfirm({ confirm: true }, "bare_motor");
    assert.equal(r.ok, true);
  });

  it("blocks weighted without confirm_weighted_motion", () => {
    const r = validateMotionConfirm(
      { confirm: true },
      "weighted_single_arm",
    );
    assert.equal(r.ok, false);
  });

  it("allows weighted with both flags", () => {
    const r = validateMotionConfirm(
      { confirm: true, confirm_weighted_motion: true },
      "weighted_single_arm",
    );
    assert.equal(r.ok, true);
  });

  it("honors profile override on args", () => {
    const r = validateMotionConfirm(
      { confirm: true, profile: "weighted_single_arm" },
      "bare_motor",
    );
    assert.equal(r.ok, false);
  });
});

describe("requiresWeightedDoubleConfirm", () => {
  it("flags weighted profiles", () => {
    assert.equal(requiresWeightedDoubleConfirm("weighted_single_arm"), true);
    assert.equal(requiresWeightedDoubleConfirm("bare_motor"), false);
  });
});
