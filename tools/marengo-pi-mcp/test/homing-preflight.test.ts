import { describe, it } from "node:test";
import assert from "node:assert/strict";
import {
  homingHealthShell,
  homingPreflightShell,
  homingStatusOutputOk,
} from "../src/homing-preflight.js";

describe("homing preflight", () => {
  it("strict shell exports HOMING_PREFLIGHT_STRICT=true", () => {
    assert.match(homingPreflightShell(true), /HOMING_PREFLIGHT_STRICT=true/);
    assert.match(homingPreflightShell(true), /homing-preflight\.sh/);
  });

  it("warn shell does not require strict exit", () => {
    assert.match(homingPreflightShell(false), /HOMING_PREFLIGHT_STRICT=false/);
  });

  it("accepts all Verified joints", () => {
    const out = [
      "right_shoulder_pitch: homing=Verified pos=0.0000 rad",
      "homing preflight: all commissioned joints Verified",
    ].join("\n");
    assert.equal(homingStatusOutputOk(out), true);
  });

  it("rejects Unhomed joints", () => {
    const out = "right_shoulder_pitch: homing=Unhomed pos=n/a";
    assert.equal(homingStatusOutputOk(out), false);
  });

  it("rejects remote exit markers", () => {
    assert.equal(homingStatusOutputOk("homing=Verified\n[exit 1]"), false);
  });

  it("pi_health includes homing preflight script", () => {
    assert.match(homingHealthShell(), /homing-preflight\.sh/);
  });
});
