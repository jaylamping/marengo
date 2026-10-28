import { describe, it } from "node:test";
import assert from "node:assert/strict";
import { execFile, spawn, spawnSync } from "node:child_process";
import { promisify } from "node:util";
import { chmodSync, mkdirSync, mkdtempSync, writeFileSync } from "node:fs";
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
import { execRemote, type RemoteExecOptions, type RemoteExecResult } from "../src/ssh.js";

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

  it("the cancel shell ends the tagged session's whole process group", async () => {
    const tag = remoteSessionTag();
    const dir = mkdtempSync(path.join(tmpdir(), "cancel-group-"));
    const marker = path.join(dir, "child-alive");
    // The session: a setsid'd `bash -s <tag>` (as sshd starts it) whose child keeps writing.
    const session = spawn("setsid", ["bash", "-s", tag], { stdio: ["pipe", "ignore", "ignore"] });
    session.stdin.end(`while true; do date +%s%N > ${marker}; sleep 0.1; done & wait\n`);
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
  });
});
