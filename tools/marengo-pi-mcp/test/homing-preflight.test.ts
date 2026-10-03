import { describe, it } from "node:test";
import assert from "node:assert/strict";
import {
  homingReportShell,
  homingStatusShell,
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

  it("homing report runs the preflight script only when CAN is free", () => {
    const report = homingReportShell();
    assert.match(report, /pgrep -l -x 'marengo-pi\|motor-repl'/);
    const [owned, free] = report.split("\nelse\n");
    assert.match(free, /homing-preflight\.sh/);
    assert.doesNotMatch(owned, /homing-preflight\.sh/);
    assert.match(owned, /snapshot\/robot\/state/);
  });

  it("homing status falls back to the gateway snapshot while CAN is owned", () => {
    const [owned, free] = homingStatusShell().split("\nelse\n");
    assert.match(free, /^bin\/motor-repl homing-status$/m);
    assert.doesNotMatch(owned, /^bin\/motor-repl/m);
    assert.match(owned, /snapshot\/robot\/state/);
  });
});
