import { describe, it } from "node:test";
import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { defaultLocalRoot } from "../src/config.js";
import {
  loadRestartMarengoPiScript,
  restartMarengoPiSchema,
  restartMarengoPiScriptPath,
  restartMarengoPiShell,
} from "../src/tools/restart-marengo-pi.js";

describe("pi_restart_marengo_pi", () => {
  const localRoot = defaultLocalRoot();
  const scriptPath = restartMarengoPiScriptPath(localRoot);
  const script = loadRestartMarengoPiScript(localRoot);

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

  it("loads the canonical scripts/pi-restart-marengo-pi.sh", () => {
    assert.match(scriptPath, /pi-restart-marengo-pi\.sh$/);
    assert.match(script, /systemctl stop marengo-pi\.service/);
    assert.match(script, /systemctl start marengo-pi\.service/);
    assert.match(script, /stop-only/);
    assert.match(script, /Hard limits \/ motors\.yaml reload/);
  });

  it("prefers passwordless installed script then embeds fallback", () => {
    const restartBody = restartMarengoPiShell("restart", script);
    assert.match(restartBody, /sudo -n "\$INSTALLED" 'restart'/);
    assert.match(restartBody, /set -- 'restart'\n/);
    assert.ok(restartBody.includes(script.replace(/^#![^\n]*\n/, "")));

    const stopBody = restartMarengoPiShell("stop", script, "/opt/marengo/scripts/pi-restart-marengo-pi.sh");
    assert.match(stopBody, /sudo -n "\$INSTALLED" 'stop'/);
    assert.match(stopBody, /set -- 'stop'\n/);
  });

  it("rejects invalid mode at the shell entrypoint", () => {
    const r = spawnSync("bash", [scriptPath, "bogus"], { encoding: "utf8" });
    assert.notEqual(r.status, 0);
    assert.match(r.stderr, /usage:.*restart\|stop/);
  });
});
