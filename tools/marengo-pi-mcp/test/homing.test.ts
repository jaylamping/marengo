import { describe, it } from "node:test";
import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { chmodSync, copyFileSync, existsSync, mkdirSync, mkdtempSync, readFileSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import path from "node:path";
import type { MarengoPiConfig } from "../src/config.js";
import { JOURNAL_TAIL_SCRIPT, NO_LIVE_SESSION_LINE, homingReportShell } from "../src/homing.js";
import { renderRobotStateHoming } from "../src/robot-state.js";
import { registerMotionTools } from "../src/tools/motion.js";
import { registerReadonlyTools } from "../src/tools/readonly.js";

const cfg: MarengoPiConfig = {
  host: "marengo.local",
  user: "joey",
  piRoot: "/opt/marengo",
  configDir: "/opt/marengo/config",
  localRoot: "/tmp/marengo",
  benchProfile: "bare_motor",
  piStagingRoot: "~/marengo",
};

const REPO_SCRIPTS = new URL("../../../scripts/", import.meta.url);

/**
 * Pi root with stub `pgrep` (marengo-pi live when `live`), `curl` (gateway down) and a
 * `bin/motor-repl` that records any invocation. Returns the root and the run's output.
 */
function runHoming(opts: { live: boolean; journalReader: boolean }) {
  const root = mkdtempSync(path.join(tmpdir(), "homing-"));
  const stubs = path.join(root, "stubs");
  mkdirSync(stubs);
  mkdirSync(path.join(root, "bin"));
  mkdirSync(path.join(root, "scripts"));
  const stub = (file: string, body: string) => {
    writeFileSync(file, `#!/usr/bin/env bash\n${body}\n`);
    chmodSync(file, 0o755);
  };
  stub(path.join(stubs, "pgrep"), `echo "$*" >>"${root}/pgrep.args"\n${opts.live ? "exit 0" : "exit 1"}`);
  stub(path.join(stubs, "curl"), "exit 22");
  stub(path.join(root, "bin", "motor-repl"), `touch "${root}/motor-repl.ran"`);
  if (opts.journalReader) {
    copyFileSync(new URL("reference-journal-tail.py", REPO_SCRIPTS), path.join(root, JOURNAL_TAIL_SCRIPT));
  }
  const r = spawnSync("bash", ["-c", `set -euo pipefail\n${homingReportShell()}`], {
    cwd: root,
    env: {
      ...process.env,
      PATH: `${stubs}:${process.env.PATH ?? ""}`,
      MARENGO_ROOT: root,
      MARENGO_REFERENCE_JOURNAL: path.join(root, "var", "reference-journal.sqlite3"),
    },
    encoding: "utf8",
  });
  assert.equal(r.status, 0, r.stderr);
  return { root, out: renderRobotStateHoming(r.stdout) };
}

describe("homing report (never opens CAN)", () => {
  it("without a live marengo-pi reports process-local grants and the journal", () => {
    const { root, out } = runHoming({ live: false, journalReader: true });
    assert.match(out, new RegExp(`^${NO_LIVE_SESSION_LINE.replace(/[()]/g, "\\$&")}$`, "m"));
    assert.match(out, /^reference journal: none at .*reference-journal\.sqlite3$/m);
    assert.doesNotMatch(out, /RobotState/);
    assert.equal(readFileSync(path.join(root, "pgrep.args"), "utf8").trim(), "-x marengo-pi");
    assert.ok(!existsSync(path.join(root, "motor-repl.ran")), "motor-repl must not run");
  });

  it("names the missing journal reader before pi_sync_main installs it", () => {
    const { root, out } = runHoming({ live: false, journalReader: false });
    assert.match(out, new RegExp(`^reference journal: ${JOURNAL_TAIL_SCRIPT} not installed`, "m"));
    assert.ok(!existsSync(path.join(root, "motor-repl.ran")), "motor-repl must not run");
  });

  it("with a live marengo-pi reads its RobotState from the gateway only", () => {
    const { root, out } = runHoming({ live: true, journalReader: true });
    assert.match(out, /^marengo-pi RobotState: unavailable/m);
    assert.doesNotMatch(out, /no live marengo-pi session|reference journal/);
    assert.ok(!existsSync(path.join(root, "motor-repl.ran")), "motor-repl must not run");
  });

  it("pi_health and pi_homing_status carry no motor-repl or CAN open", async () => {
    const scripts: string[] = [];
    const record = async (body: string) => {
      scripts.push(body);
      return "robot_state_b64=";
    };
    await registerReadonlyTools(cfg, record).pi_health.handler();
    await registerMotionTools(cfg, record, () => {}).pi_homing_status.handler({});
    assert.equal(scripts.length, 2);
    for (const script of scripts) {
      assert.ok(script.includes(homingReportShell()), "uses the shared no-CAN homing report");
      assert.doesNotMatch(script, /bin\/motor-repl|homing-preflight|homing-status|can-up/);
    }
  });
});
