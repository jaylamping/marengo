import { describe, it } from "node:test";
import assert from "node:assert/strict";
import { execFile, spawn, spawnSync } from "node:child_process";
import { promisify } from "node:util";
import { chmodSync, existsSync, mkdirSync, mkdtempSync, readFileSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import path from "node:path";
import type { MarengoPiConfig } from "../src/config.js";
import {
  CANCELLED_TEXT,
  callContext,
  cancelMotionSessionShell,
  makeRunRemote,
  remoteSessionTag,
} from "../src/cancel.js";
import { wrapRemoteWithConfig } from "../src/env.js";
import { execRemote, type RemoteExecOptions, type RemoteExecResult } from "../src/ssh.js";
import { marengoPiPipe } from "../src/tools/motion.js";

const execFileAsync = promisify(execFile);

const cfg: MarengoPiConfig = {
  host: "marengo.local",
  user: "joey",
  piRoot: "/opt/marengo",
  configDir: "/opt/marengo/config",
  localRoot: "/tmp/marengo",
  benchProfile: "bare_motor",
  piStagingRoot: "~/marengo",
};

/** Fake ssh on PATH: runs the remote script locally, recording its argv. */
function fakeSshPath(): { dir: string; env: NodeJS.ProcessEnv } {
  const dir = mkdtempSync(path.join(tmpdir(), "fake-ssh-"));
  const bin = path.join(dir, "bin");
  mkdirSync(bin);
  writeFileSync(path.join(bin, "ssh"), `#!/bin/bash\necho "$*" > "${dir}/argv"\nexec bash -s\n`);
  chmodSync(path.join(bin, "ssh"), 0o755);
  return { dir, env: { ...process.env, PATH: `${bin}:${process.env.PATH}` } };
}

describe("MCP cancel (TOOL-03)", () => {
  it("execRemote kills the running ssh on abort and reports it", async () => {
    const { dir, env } = fakeSshPath();
    const saved = process.env.PATH;
    process.env.PATH = env.PATH;
    try {
      const abort = new AbortController();
      const started = Date.now();
      const pending = execRemote(cfg, "exec sleep 30", { signal: abort.signal, sessionTag: "marengo-mcp-t" });
      setTimeout(() => abort.abort(), 200);
      const r = await pending;
      assert.equal(r.cancelled, true);
      assert.equal(r.exitCode, 130);
      assert.ok(Date.now() - started < 10_000, "did not wait for the remote sleep");
      const argv = spawnSync("cat", [path.join(dir, "argv")], { encoding: "utf8" }).stdout;
      assert.match(argv, /bash -s marengo-mcp-t$/m);
    } finally {
      process.env.PATH = saved;
    }
  });

  it("a cancelled motion call runs the stop; later calls start nothing", async () => {
    const calls: Array<{ body: string; opts: RemoteExecOptions }> = [];
    const abort = new AbortController();
    const fake = async (_cfg: MarengoPiConfig, body: string, opts: RemoteExecOptions = {}): Promise<RemoteExecResult> => {
      calls.push({ body, opts });
      if (calls.length === 1) {
        abort.abort();
        return { stdout: "", stderr: "cancelled", exitCode: 130, cancelled: true };
      }
      return { stdout: "stopped", stderr: "", exitCode: 0 };
    };
    const runRemote = makeRunRemote(cfg, fake as typeof execRemote);
    await callContext.run({ signal: abort.signal, motion: true }, async () => {
      const first = await runRemote("marengo-pi session");
      assert.match(first, /--- cancel stop ---/);
      assert.equal(calls.length, 2);
      assert.equal(calls[1].opts.signal, undefined, "the stop itself cannot be cancelled");
      assert.ok(calls[1].body.includes(`bash -s ${calls[0].opts.sessionTag}`));
      assert.match(calls[1].body, /bin\/motor-repl disable/);
      assert.equal(await runRemote("next part"), `${CANCELLED_TEXT}\n\n[exit 130]`);
      assert.equal(calls.length, 2);
    });
  });

  it("a non-motion call in flight is not given the signal", async () => {
    let seen: AbortSignal | undefined = new AbortController().signal;
    const fake = async (_c: MarengoPiConfig, _b: string, opts: RemoteExecOptions = {}) => {
      seen = opts.signal;
      return { stdout: "", stderr: "", exitCode: 0 };
    };
    await callContext.run({ signal: new AbortController().signal, motion: false }, () =>
      makeRunRemote(cfg, fake as typeof execRemote)("install"),
    );
    assert.equal(seen, undefined);
  });

  // The Pi has procps; a build image without pgrep/ps cannot run the shell under test.
  const procps = ["pgrep", "ps"].every((tool) => spawnSync("sh", ["-c", `command -v ${tool}`]).status === 0);

  it("the cancel shell ends the tagged session's whole process group", { skip: !procps && "needs procps" }, async () => {
    const tag = remoteSessionTag();
    const dir = mkdtempSync(path.join(tmpdir(), "cancel-group-"));
    const marker = path.join(dir, "child-alive");
    // The session: `bash -s <tag>` leading its own session (as sshd starts it) whose child
    // keeps writing. The loop ends by itself after 30 s so a failed kill cannot hang the run.
    const session = spawn("bash", ["-s", tag], { detached: true, stdio: ["pipe", "ignore", "ignore"] });
    session.stdin.end(`for _ in $(seq 300); do date +%s%N > ${marker}; sleep 0.1; done & wait\n`);
    try {
      await new Promise((r) => setTimeout(r, 400));
      // Run only the group-kill part; the sole-owner stop needs a Pi.
      const shell = cancelMotionSessionShell(tag).split("\npkill -x motor-repl")[0];
      // Async, so node reaps the session shell (sshd does on the Pi) and pgrep -g sees no zombie.
      const r = await execFileAsync("bash", ["-c", `set -euo pipefail\n${shell}`], { encoding: "utf8" });
      assert.match(r.stdout, /cancel: ending remote session \d+ \(group \d+\)/);
      const left = spawnSync("pgrep", ["-f", "-x", `bash -s ${tag}`]);
      assert.notEqual(left.status, 0, "session shell is gone");
      const before = spawnSync("cat", [marker], { encoding: "utf8" }).stdout;
      await new Promise((r) => setTimeout(r, 300));
      assert.equal(spawnSync("cat", [marker], { encoding: "utf8" }).stdout, before, "child stopped writing");
    } finally {
      try {
        if (session.pid !== undefined) process.kill(-session.pid, "SIGKILL");
      } catch {
        // Already gone: the cancel shell did its job.
      }
    }
  });
  // The whole cancel stop as makeRunRemote sends it, run locally: systemctl, sudo and pkill
  // are stubs that log, and bin/motor-repl under a temporary piRoot logs its config dir.
  function cancelStopRig(opts: { pgrepFindsGoneSession?: boolean } = {}) {
    const dir = mkdtempSync(path.join(tmpdir(), "cancel-stop-"));
    const stubs = path.join(dir, "stubs");
    const piRoot = path.join(dir, "opt");
    const sysfs = path.join(dir, "sysfs");
    const log = path.join(dir, "log");
    for (const d of [stubs, path.join(piRoot, "bin"), sysfs]) mkdirSync(d, { recursive: true });
    const stub = (file: string, body: string) => {
      writeFileSync(file, `#!/bin/bash\n${body}\n`);
      chmodSync(file, 0o755);
    };
    stub(path.join(stubs, "systemctl"), 'echo "${STUB_UNIT_STATE:-inactive}"; [[ "${STUB_UNIT_STATE:-inactive}" == active ]]');
    stub(path.join(stubs, "sudo"), `echo "sudo $*" >> ${log}`);
    stub(path.join(stubs, "pkill"), `echo "pkill $*" >> ${log}; exit 1`);
    stub(path.join(piRoot, "bin", "motor-repl"), `echo "motor-repl $* config=$MARENGO_CONFIG_DIR" >> ${log}`);
    if (opts.pgrepFindsGoneSession) {
      // The session ends between pgrep and ps: pgrep still names it, ps finds nothing.
      const real = spawnSync("sh", ["-c", "command -v pgrep"], { encoding: "utf8" }).stdout.trim();
      stub(path.join(stubs, "pgrep"), `if [[ "$1" == -f ]]; then echo 999999; exit 0; fi\nexec ${real} "$@"`);
    }
    const rigCfg: MarengoPiConfig = { ...cfg, piRoot, configDir: "/default/config" };
    const run = async (sessionBody: string, sessionStdout: string, earlierSteps: string[] = []) => {
      const calls: string[] = [];
      const abort = new AbortController();
      const fake = async (_c: MarengoPiConfig, body: string): Promise<RemoteExecResult> => {
        calls.push(body);
        if (calls.length <= earlierSteps.length) {
          return { stdout: earlierSteps[calls.length - 1], stderr: "", exitCode: 0 };
        }
        if (calls.length === earlierSteps.length + 1) {
          abort.abort();
          return { stdout: sessionStdout, stderr: "", exitCode: 130, cancelled: true };
        }
        return { stdout: "", stderr: "", exitCode: 0 };
      };
      await callContext.run({ signal: abort.signal, motion: true }, async () => {
        const runRemote = makeRunRemote(rigCfg, fake as typeof execRemote);
        for (const _ of earlierSteps) await runRemote("earlier step");
        await runRemote(sessionBody);
      });
      assert.equal(calls.length, earlierSteps.length + 2, "the cancel stop ran");
      const r = spawnSync("bash", ["-c", calls[calls.length - 1]], {
        encoding: "utf8",
        env: {
          ...process.env,
          PATH: `${stubs}:${process.env.PATH}`,
          MARENGO_CAN_SYSFS: sysfs,
        },
      });
      const logged = existsSync(log) ? readFileSync(log, "utf8").trim().split("\n") : [];
      return { status: r.status, stdout: r.stdout, stderr: r.stderr, log: logged };
    };
    return { rigCfg, run };
  }

  const RESTORE_HELPER = "sudo -n /usr/local/libexec/marengo/pi-restart-marengo-pi.sh";

  it("the backstop stop still runs when the session ends between pgrep and ps", { skip: !procps && "needs procps" }, async () => {
    const { rigCfg, run } = cancelStopRig({ pgrepFindsGoneSession: true });
    const r = await run(wrapRemoteWithConfig(rigCfg, "true"), "");
    assert.equal(r.status, 0, `${r.stdout}\n${r.stderr}`);
    assert.ok(r.log.some((l) => l.startsWith("motor-repl disable")), r.log.join("\n"));
    assert.ok(r.log.includes("pkill -x motor-repl"), r.log.join("\n"));
  });

  it("restores marengo-pi.service that the cancelled session stopped", { skip: !procps && "needs procps" }, async () => {
    // The session took the unit down; its EXIT trap never ran, so the unit reads inactive now.
    const { rigCfg, run } = cancelStopRig();
    const session = "marengo-pi.service restore after session: true\n=== session ===\n";
    const r = await run(wrapRemoteWithConfig(rigCfg, "true"), session);
    assert.equal(r.status, 0, `${r.stdout}\n${r.stderr}`);
    const disable = r.log.findIndex((l) => l.startsWith("motor-repl disable"));
    const restart = r.log.indexOf(`${RESTORE_HELPER} restart`);
    assert.ok(disable >= 0 && restart > disable, `drives stop first, then the unit restarts:\n${r.log.join("\n")}`);
  });

  it("restores the unit an earlier step of the same call stopped (harness)", { skip: !procps && "needs procps" }, async () => {
    const { rigCfg, run } = cancelStopRig();
    const r = await run(wrapRemoteWithConfig(rigCfg, "true"), "=== session ===\n", [
      "marengo-pi.service restore after session: true\n",
    ]);
    assert.equal(r.status, 0, `${r.stdout}\n${r.stderr}`);
    assert.ok(r.log.includes(`${RESTORE_HELPER} restart`), r.log.join("\n"));
  });

  it("does not start marengo-pi.service when the session found it stopped", { skip: !procps && "needs procps" }, async () => {
    const { rigCfg, run } = cancelStopRig();
    const r = await run(wrapRemoteWithConfig(rigCfg, "true"), "marengo-pi.service restore after session: false\n");
    assert.equal(r.status, 0, `${r.stdout}\n${r.stderr}`);
    assert.ok(!r.log.includes(`${RESTORE_HELPER} restart`), r.log.join("\n"));
  });

  it("the backstop disables the drives of the session's config dir, and the default's", { skip: !procps && "needs procps" }, async () => {
    const { rigCfg, run } = cancelStopRig();
    const r = await run(wrapRemoteWithConfig(rigCfg, "true", "/opt/marengo/config/bringup/it's"), "");
    assert.equal(r.status, 0, `${r.stdout}\n${r.stderr}`);
    const disables = r.log.filter((l) => l.startsWith("motor-repl disable"));
    assert.deepEqual(disables, [
      "motor-repl disable config=/opt/marengo/config/bringup/it's",
      "motor-repl disable config=/default/config",
    ]);
  });

  it("the marengo-pi pipe timeout still ends a session that overruns it", () => {
    const started = Date.now();
    const r = spawnSync("bash", ["-c", `set -o pipefail\n${marengoPiPipe(["sleep 1"], 1, "bash -c 'exec 0</dev/null; exec sleep 30'")}`], {
      encoding: "utf8",
      timeout: 20_000,
    });
    assert.equal(r.status, 124, "timeout(1) expired and reported it");
    assert.ok(Date.now() - started < 10_000, "the cap held");
  });

  it("the group SIGTERM reaches marengo-pi under its timeout(1) wrapper", { skip: !procps && "needs procps" }, async () => {
    const tag = remoteSessionTag();
    const dir = mkdtempSync(path.join(tmpdir(), "cancel-timeout-"));
    const mark = path.join(dir, "marengo-pi-exit");
    // Stands in for marengo-pi: it ignores stdin EOF, so only a signal stops it early.
    const child = path.join(dir, "fake-marengo-pi");
    writeFileSync(
      child,
      `#!/bin/bash\ntrap 'echo TERM > ${mark}; exit 0' TERM\nexec 0</dev/null\nfor _ in $(seq 200); do sleep 0.1; done\necho TIMEOUT > ${mark}\n`,
    );
    chmodSync(child, 0o755);
    const session = spawn("bash", ["-s", tag], { detached: true, stdio: ["pipe", "ignore", "ignore"] });
    session.stdin.end(`${marengoPiPipe(["sleep 20"], 20, child)}\n`);
    try {
      await new Promise((r) => setTimeout(r, 400));
      const shell = cancelMotionSessionShell(tag).split("\npkill -x motor-repl")[0];
      await execFileAsync("bash", ["-c", `set -euo pipefail\n${shell}`], { encoding: "utf8" });
      for (let i = 0; i < 20 && !existsSync(mark); i++) await new Promise((r) => setTimeout(r, 100));
      assert.equal(existsSync(mark) ? readFileSync(mark, "utf8").trim() : "still running", "TERM");
    } finally {
      try {
        if (session.pid !== undefined) process.kill(-session.pid, "SIGKILL");
      } catch {
        // Already gone.
      }
      spawnSync("pkill", ["-KILL", "-f", dir]);
    }
  });
});
