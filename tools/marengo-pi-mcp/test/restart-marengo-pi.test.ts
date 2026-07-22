import { describe, it } from "node:test";
import assert from "node:assert/strict";
import {
  restartMarengoPiSchema,
  restartMarengoPiShell,
} from "../src/tools/restart-marengo-pi.js";

describe("pi_restart_marengo_pi", () => {
  it("requires confirm: true", () => {
    assert.throws(() => restartMarengoPiSchema.parse({ confirm: false }));
    assert.throws(() => restartMarengoPiSchema.parse({}));
  });

  it("defaults mode to restart", () => {
    const parsed = restartMarengoPiSchema.parse({ confirm: true });
    assert.equal(parsed.mode, "restart");
    assert.equal(parsed.confirm, true);
  });

  it("accepts stop mode", () => {
    const parsed = restartMarengoPiSchema.parse({ confirm: true, mode: "stop" });
    assert.equal(parsed.mode, "stop");
  });

  it("restart shell stops then starts the unit", () => {
    const shell = restartMarengoPiShell("restart");
    assert.match(shell, /systemctl stop marengo-pi\.service/);
    assert.match(shell, /pkill -f '\/opt\/marengo\/bin\/marengo-pi'/);
    assert.match(shell, /systemctl start marengo-pi\.service/);
    assert.doesNotMatch(shell, /stop-only/);
  });

  it("stop shell does not start the unit", () => {
    const shell = restartMarengoPiShell("stop");
    assert.match(shell, /systemctl stop marengo-pi\.service/);
    assert.match(shell, /stop-only/);
    assert.doesNotMatch(shell, /systemctl start marengo-pi\.service/);
  });
});
