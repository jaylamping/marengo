import { describe, it } from "node:test";
import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { chmodSync, mkdirSync, mkdtempSync, readFileSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import path from "node:path";
import type { MarengoPiConfig } from "../src/config.js";
import { exitCodeOfRemoteOutput, formatRemoteResult } from "../src/ssh.js";
import {
  benchLogWrapper,
  registerMotionTools,
  zeroActuatorRemoteBody,
} from "../src/tools/motion.js";

const cfg: MarengoPiConfig = {
  host: "marengo.local",
  user: "joey",
  piRoot: "/opt/marengo",
  configDir: "/opt/marengo/config",
  localRoot: "/tmp/marengo",
  benchProfile: "bare_motor",
  piStagingRoot: "~/marengo",
};

describe("exit code carried by remote output", () => {
  it("reads the [exit N] marker formatRemoteResult appends", () => {
    const failed = formatRemoteResult({ stdout: "disable can0:1 sent\n", stderr: "x", exitCode: 1 });
    assert.equal(exitCodeOfRemoteOutput(failed), 1);
    const ok = formatRemoteResult({ stdout: "disabled\n", stderr: "", exitCode: 0 });
    assert.equal(exitCodeOfRemoteOutput(ok), 0);
    assert.equal(exitCodeOfRemoteOutput("note: [exit 7] in the middle\nmore"), 0);
  });
});

describe("stop tools report a failed stop", () => {
  it("audits pi_motor_disable and pi_hold_off with the real exit code", async () => {
    const audits: Array<[string, number]> = [];
    const tools = registerMotionTools(
      cfg,
      async () => "disable can0:3 FAILED: open: No such device\n\n[exit 1]",
      (tool, _args, _result, exitCode) => audits.push([tool, exitCode]),
    );
    await tools.pi_motor_disable.handler({ confirm: true });
    await tools.pi_hold_off.handler({ confirm: true });
    assert.deepEqual(audits, [
      ["pi_motor_disable", 1],
      ["pi_hold_off", 1],
    ]);
  });

  it("does not call Disable a fault clear", () => {
    const tools = registerMotionTools(cfg, async () => "", () => {});
    const description = tools.pi_motor_disable.description;
    assert.doesNotMatch(description, /primary fault clear/i);
    assert.match(description, /does NOT clear a latched drive fault/);
  });
});

describe("no motion session starts behind an unconfirmed stop", () => {
  it("benchLogWrapper aborts before marengo-pi when the pre-session disable fails", () => {
    const script = benchLogWrapper(cfg, "echo PIPE", "test");
    assert.match(script, /if ! bin\/motor-repl disable; then/);
    assert.match(script, /PRE-SESSION DISABLE INCOMPLETE[^\n]*marengo-pi NOT launched/);
    assert.doesNotMatch(script, /bin\/motor-repl disable[^\n]*\|\| true/);
    assert.doesNotMatch(script, /bin\/motor-repl disable[^\n]*2>\/dev\/null/);
    const abort = script.indexOf("PRE-SESSION DISABLE INCOMPLETE");
    assert.ok(abort > 0 && abort < script.indexOf("echo PIPE"), "abort precedes the session body");
  });

  it("pi_motor_recover fails RECOVER when the disable did not reach every drive", async () => {
    let script = "";
    const tools = registerMotionTools(
      cfg,
      async (body) => {
        script = body;
        return body;
      },
      () => {},
    );
    await tools.pi_motor_recover.handler({ confirm: true });
    assert.doesNotMatch(script, /motor-repl disable 2>\/dev\/null \|\| true/);
    assert.match(script, /RECOVER_DISABLE_INCOMPLETE/);
    assert.match(script, /grep -q "RECOVER_DISABLE_INCOMPLETE"[\s\S]*RECOVER_FAIL: disable did not reach every drive/);
  });
});

describe("pi_set_zero body keeps both outcomes", () => {
  function run(motorRepl: string) {
    const dir = mkdtempSync(path.join(tmpdir(), "zero-body-"));
    mkdirSync(path.join(dir, "bin"));
    const trace = path.join(dir, "trace");
    writeFileSync(trace, "");
    writeFileSync(path.join(dir, "bin", "motor-repl"), `#!/bin/bash\necho "$*" >> "${trace}"\n${motorRepl}\n`);
    chmodSync(path.join(dir, "bin", "motor-repl"), 0o755);
    // Same strict mode the remote preamble sets.
    const r = spawnSync("bash", ["-c", `set -euo pipefail\n${zeroActuatorRemoteBody("right_elbow_pitch")}`], {
      cwd: dir,
      encoding: "utf8",
    });
    return { ...r, trace: readFileSync(trace, "utf8").trim().split("\n") };
  }

  it("disables even when set-zero fails, and still reports the set-zero failure", () => {
    const r = run('[ "$1" = set-zero ] && exit 3\nexit 0');
    assert.equal(r.status, 3);
    assert.deepEqual(r.trace, ["set-zero right_elbow_pitch --sign-tested", "disable"]);
  });

  it("reports an incomplete disable after a good set-zero", () => {
    const r = run('[ "$1" = disable ] && exit 1\nexit 0');
    assert.equal(r.status, 1);
    assert.match(r.stdout, /POST-SET-ZERO DISABLE INCOMPLETE/);
  });

  it("exits 0 when both succeed", () => {
    const r = run("exit 0");
    assert.equal(r.status, 0);
  });
});
