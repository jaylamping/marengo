import { describe, it } from "node:test";
import assert from "node:assert/strict";
import type { MarengoPiConfig } from "../src/config.js";
import { deployRevLogCommand, localDeployCommand, runSyncMain } from "../src/tools/deploy.js";
import type { RemoteExecResult } from "../src/ssh.js";
import { toolResultIsError } from "../src/cancel.js";

const cfg: MarengoPiConfig = {
  host: "marengo.local",
  user: "joey",
  piRoot: "/opt/marengo",
  configDir: "/opt/marengo/config",
  localRoot: "/tmp/marengo",
  benchProfile: "bare_motor",
  piStagingRoot: "~/marengo",
};

describe("deploy rev log", () => {
  it("reads canonical /opt/marengo/.deploy-rev", () => {
    const cmd = deployRevLogCommand("/opt/marengo");
    assert.match(cmd, /cat '\/opt\/marengo\/\.deploy-rev'/);
    assert.match(cmd, /\(no \.deploy-rev\)/);
  });

  it("quotes custom install roots", () => {
    const cmd = deployRevLogCommand("/opt/marengo-test");
    assert.match(cmd, /\/opt\/marengo-test\/\.deploy-rev/);
  });
});

describe("pi_sync_main cross", () => {
  // Management gate passes: no marengo-pi, no motor-repl on the Pi.
  const gateOk = async () => "mgmt_marengo_pi=0\nmgmt_motor_repl=0\n";

  function fakeGit(replies: Record<string, string>) {
    const calls: string[] = [];
    const local = async (cmd: string, args: string[]): Promise<RemoteExecResult> => {
      const line = [cmd, ...args].join(" ");
      calls.push(line);
      const key = Object.keys(replies).find((k) => line.startsWith(k));
      return { stdout: key ? replies[key] : "", stderr: "", exitCode: 0 };
    };
    return { calls, local };
  }

  it("never installs from the build step: runSyncMain runs install-pi.sh once (TOOL-13)", () => {
    for (const platform of ["darwin", "linux", "win32"] as const) {
      const { args } = localDeployCommand(cfg, platform);
      assert.ok(!args.includes("--install"), `${platform}: ${args.join(" ")}`);
      assert.equal(args[args.length - 1], "joey@marengo.local");
    }
    assert.ok(localDeployCommand(cfg, "win32").args.includes("-NoInstall"));
  });

  it("refuses on a branch other than main and never switches it (TOOL-12)", async () => {
    const { calls, local } = fakeGit({ "git rev-parse --abbrev-ref HEAD": "feature-x\n" });
    const out = await runSyncMain(cfg, gateOk, "cross", {}, local);
    assert.match(out, /^Error: local checkout is on 'feature-x', not main/);
    assert.ok(!calls.some((c) => /checkout|pull|fetch|deploy-pi/.test(c)), calls.join("\n"));
  });

  it("treats untracked files in shipped trees as dirty", async () => {
    const { calls, local } = fakeGit({
      "git rev-parse --abbrev-ref HEAD": "main\n",
      "git status --porcelain --untracked-files=all -- config assets scripts": "?? config/scratch.yaml\n",
    });
    const out = await runSyncMain(cfg, gateOk, "cross", {}, local);
    assert.match(out, /^Local repo dirty[\s\S]*config\/scratch\.yaml/);
    assert.ok(!calls.some((c) => /pull|fetch|deploy-pi/.test(c)), calls.join("\n"));
  });
});

describe("pi_sync_main cross step failures (TOOL-13)", () => {
  const gateOk = async () => "mgmt_marengo_pi=0\nmgmt_motor_repl=0\n";
  const ok: RemoteExecResult = { stdout: "", stderr: "", exitCode: 0 };

  async function localOk(cmd: string, args: string[]): Promise<RemoteExecResult> {
    const line = [cmd, ...args].join(" ");
    if (line === "git rev-parse --abbrev-ref HEAD") return { ...ok, stdout: "main\n" };
    if (line === "git rev-parse HEAD") return { ...ok, stdout: `${"a".repeat(40)}\n` };
    return ok;
  }

  it("a failed install-pi.sh fails the tool (isError) and stops before verify", async () => {
    const remoteCalls: string[] = [];
    const remote = async (_cfg: MarengoPiConfig, body: string): Promise<RemoteExecResult> => {
      remoteCalls.push(body);
      if (body.includes("install-pi.sh")) {
        return { stdout: "", stderr: "install-pi.sh: boom", exitCode: 1 };
      }
      return ok;
    };
    const out = await runSyncMain(cfg, gateOk, "cross", { waitForReady: false }, localOk, remote);
    assert.match(out, /^Error: install-pi\.sh failed/);
    assert.match(out, /install-pi\.sh: boom/);
    assert.equal(toolResultIsError(out, { motion: false }), true);
    assert.equal(remoteCalls.length, 1, remoteCalls.join("\n---\n"));
  });

  it("a failed local build fails the tool before any remote step", async () => {
    const remoteCalls: string[] = [];
    const remote = async (_cfg: MarengoPiConfig, body: string): Promise<RemoteExecResult> => {
      remoteCalls.push(body);
      return ok;
    };
    const local = async (cmd: string, args: string[]): Promise<RemoteExecResult> =>
      args.some((a) => a.includes("deploy-pi")) ? { ...ok, exitCode: 2 } : localOk(cmd, args);
    const out = await runSyncMain(cfg, gateOk, "cross", { waitForReady: false }, local, remote);
    assert.match(out, /^Error: build failed/);
    assert.equal(toolResultIsError(out, { motion: false }), true);
    assert.equal(remoteCalls.length, 0);
  });

  it("pi_native: a failed pi-self-update.sh fails the tool", async () => {
    const remote = async (_cfg: MarengoPiConfig, body: string): Promise<RemoteExecResult> =>
      body.includes("pi-self-update.sh")
        ? { stdout: "", stderr: "self-update: boom", exitCode: 1 }
        : { ...ok, stdout: `${"b".repeat(40)}\n` };
    const out = await runSyncMain(cfg, gateOk, "pi_native", { waitForReady: false }, localOk, remote);
    assert.match(out, /^Error: pi-self-update\.sh failed[\s\S]*self-update: boom/);
    assert.equal(toolResultIsError(out, { motion: false }), true);
  });

  it("a clean run is not an error", async () => {
    const remote = async (): Promise<RemoteExecResult> => ok;
    const out = await runSyncMain(cfg, gateOk, "cross", { waitForReady: false }, localOk, remote);
    assert.match(out, /^\[local git fetch\]/);
    assert.equal(toolResultIsError(out, { motion: false }), false);
  });
});
