import { describe, it } from "node:test";
import assert from "node:assert/strict";
import { homingReportShell, homingStatusShell } from "../src/homing-preflight.js";

describe("homing preflight", () => {
  it("homing report runs the preflight script warn-only", () => {
    assert.match(homingReportShell(), /export HOMING_PREFLIGHT_STRICT=false\n\.\/scripts\/homing-preflight\.sh/);
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
