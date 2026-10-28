import path from "node:path";
import type { MarengoPiConfig } from "../config.js";
import { sudoStagingInstallCommand } from "../config.js";
import { shellQuote, wrapRemote, wrapStagingRemote } from "../env.js";
import { execLocal, execRemote, formatRemoteResult } from "../ssh.js";
import { waitForDeployReady } from "./deploy-wait.js";
import { checkManagementGate } from "../management-gate.js";

/**
 * Local cross build + rsync to the Pi staging tree. It never installs: runSyncMain runs
 * install-pi.sh once itself and fails on its exit, where `deploy-pi.sh --install` only
 * warns (TOOL-13).
 */
export function localDeployCommand(
  cfg: MarengoPiConfig,
  platform: NodeJS.Platform = process.platform,
): { cmd: string; args: string[] } {
  const host = `${cfg.user}@${cfg.host}`;
  if (platform === "win32") {
    // Windows: run the PowerShell port directly so docker.exe uses the
    // desktop-linux npipe context (bash on Windows hits "protocol not
    // available" via an incompatible context).
    return {
      cmd: "pwsh",
      args: [
        "-NoProfile",
        "-ExecutionPolicy",
        "Bypass",
        "-File",
        "./scripts/deploy-pi-docker.ps1",
        "-NoInstall",
        host,
      ],
    };
  }
  return {
    cmd: "bash",
    args: ["./scripts/deploy-pi.sh", host],
  };
}

/**
 * Result text for a step that failed: `Error: <step> failed` heads the step log, so the MCP
 * result carries isError (toolResultIsError) and the client never reads it as a deploy.
 */
function stepFailed(step: string, steps: string[], sep = "\n\n"): string {
  return `Error: ${step} failed\n\n${steps.join(sep)}`;
}

/** Trees deploy-pi.sh copies wholesale into the bundle: untracked files there would ship. */
const SHIPPED_TREES = ["config", "assets", "scripts"];

export async function runSyncMain(
  cfg: MarengoPiConfig,
  runRemote: (body: string, timeoutMs?: number) => Promise<string>,
  strategy: "cross" | "pi_native",
  opts: { waitForReady?: boolean; waitTimeoutSec?: number } = {},
  local: typeof execLocal = execLocal,
  remote: typeof execRemote = execRemote,
): Promise<string> {
  const waitForReady = opts.waitForReady ?? true;
  const waitTimeoutSec = opts.waitTimeoutSec ?? 180;
  const steps: string[] = [];
  // Both strategies end in install-pi.sh, which stops marengo-pi (P2B-05).
  const refusal = await checkManagementGate(cfg, runRemote, "pi_sync_main");
  if (refusal) return refusal;

  if (strategy === "pi_native") {
    // Canonical path: scripts/pi-self-update.sh (shared with Consul /control/deploy).
    // Resolve the tip in the ~/marengo checkout, never the sealed /opt install tree.
    const tipBody = wrapStagingRemote(
      cfg,
      [
        "git fetch origin",
        "git rev-parse origin/main",
      ].join("\n"),
    );
    const tipR = await remote(cfg, tipBody, { timeoutMs: 120_000 });
    steps.push(`[resolve origin/main]\n${formatRemoteResult(tipR)}`);
    if (tipR.exitCode !== 0) return stepFailed("resolve origin/main", steps, "\n\n---\n\n");
    const head = tipR.stdout.trim().split(/\s+/).pop() ?? "";
    if (!/^[0-9a-f]{7,40}$/i.test(head)) {
      steps.push(`[resolve origin/main] error: bad SHA '${head}'`);
      return stepFailed("resolve origin/main", steps, "\n\n---\n\n");
    }
    steps.push(`[deploy rev] ${head}`);

    const body = wrapRemote(
      cfg,
      [
        `export TARGET_SHA=${shellQuote(head)}`,
        `export JOB_ID=mcp-$(date -u +%Y%m%dT%H%M%SZ)`,
        'export JOB_FILE="${MARENGO_DEPLOY_JOB_FILE:-/opt/marengo/var/deploy-job.json}"',
        'export MARENGO_STAGING_ROOT="${MARENGO_STAGING_ROOT:-$HOME/marengo}"',
        'mkdir -p "$(dirname "$JOB_FILE")" 2>/dev/null || true',
        'if [[ -x ./scripts/pi-self-update.sh ]]; then',
        "  ./scripts/pi-self-update.sh",
        "elif [[ -x scripts/pi-self-update.sh ]]; then",
        "  ./scripts/pi-self-update.sh",
        "else",
        "  echo 'error: scripts/pi-self-update.sh missing' >&2",
        "  exit 127",
        "fi",
      ].join("\n"),
    );
    const r = await remote(cfg, body, { timeoutMs: 900_000 });
    steps.push(formatRemoteResult(r));
    if (r.exitCode !== 0) return stepFailed("pi-self-update.sh", steps, "\n\n---\n\n");

    await logDeployRev(cfg, runRemote, steps, head || undefined, remote);

    if (waitForReady && head) {
      steps.push(
        `[wait for gateway] polling up to ${waitTimeoutSec}s for rev ${head.slice(0, 12)}…`,
      );
      const wait = await waitForDeployReady(cfg, head, {
        timeoutMs: waitTimeoutSec * 1000,
      });
      steps.push(wait.log);
      if (!wait.ready) {
        steps.push(
          `[wait for gateway] TIMEOUT — Pi not ready after ${waitTimeoutSec}s (check marengo-gateway / www)`,
        );
      }
    }

    return steps.join("\n\n---\n\n");
  }

  // cross-build from Mac. It builds the local checkout, so that must already be a clean main:
  // the tool never switches the user's branch (TOOL-12).
  const branch = await local("git", ["rev-parse", "--abbrev-ref", "HEAD"], { cwd: cfg.localRoot });
  const current = branch.stdout.trim();
  if (branch.exitCode !== 0 || current !== "main") {
    return (
      `Error: local checkout is on '${current || "unknown"}', not main. pi_sync_main cross builds ` +
      "the local checkout and never switches branches: check out main yourself, or use " +
      "strategy pi_native (builds origin/main on the Pi)."
    );
  }
  const status = await local(
    "git",
    ["status", "--porcelain", "--untracked-files=no"],
    { cwd: cfg.localRoot },
  );
  const untracked = await local(
    "git",
    ["status", "--porcelain", "--untracked-files=all", "--", ...SHIPPED_TREES],
    { cwd: cfg.localRoot },
  );
  const dirty = `${status.stdout.trim()}\n${untracked.stdout.trim()}`.trim();
  if (dirty) {
    return `Local repo dirty — commit or stash first:\n${dirty}`;
  }

  const fetch = await local(
    "git",
    ["fetch", "origin", "main"],
    { cwd: cfg.localRoot, timeoutMs: 120_000 },
  );
  steps.push(`[local git fetch]\n${formatRemoteResult(fetch)}`);
  if (fetch.exitCode !== 0) return stepFailed("local git fetch", steps);

  const pull = await local(
    "git",
    ["pull", "--ff-only", "origin", "main"],
    { cwd: cfg.localRoot, timeoutMs: 120_000 },
  );
  steps.push(`[local pull]\n${formatRemoteResult(pull)}`);
  if (pull.exitCode !== 0) return stepFailed("local pull", steps);

  const rev = await local("git", ["rev-parse", "HEAD"], {
    cwd: cfg.localRoot,
  });
  const head = rev.stdout.trim();
  steps.push(`[deploy rev] ${head}`);

  const deploy = localDeployCommand(cfg);
  const deployResult = await local(deploy.cmd, deploy.args, {
    cwd: cfg.localRoot,
    timeoutMs: 900_000,
  });
  steps.push(
    `[${path.basename(deploy.args[0])}]\n${formatRemoteResult(deployResult)}`,
  );
  if (deployResult.exitCode !== 0) return stepFailed("build", steps);

  const installBody = wrapRemote(cfg, sudoStagingInstallCommand(cfg));
  const installOpt = await remote(cfg, installBody, { timeoutMs: 120_000 });
  steps.push(`[install-pi.sh staging → /opt]\n${formatRemoteResult(installOpt)}`);
  if (installOpt.exitCode !== 0) return stepFailed("install-pi.sh", steps);

  const staging = cfg.piStagingRoot.replace(/^~/, `/home/${cfg.user}`);
  const verifyBody = [
    "set -euo pipefail",
    `cd ${shellQuote(staging)}`,
    "if pgrep -af marengo-pi >/dev/null 2>&1; then echo 'warning: marengo-pi running' >&2; fi",
    // Never execute marengo-pi here: without /etc/marengo/env it resolves the
    // compile-time workspace root (deploy host path); with it, it starts control.
    "test -x /opt/marengo/bin/marengo-pi",
    "echo \"deploy rev: $(cat /opt/marengo/.deploy-rev 2>/dev/null || echo missing)\"",
    "echo 'install verified'",
  ].join("\n");
  const verify = await remote(cfg, verifyBody, { timeoutMs: 120_000 });
  steps.push(`[verify install]\n${formatRemoteResult(verify)}`);
  if (verify.exitCode !== 0) return stepFailed("verify install", steps);

  await logDeployRev(cfg, runRemote, steps, head, remote);

  const healthBody = wrapRemote(
    cfg,
    "test -x bin/marengo-pi && echo 'install ok'",
  );
  const health = await remote(cfg, healthBody, { timeoutMs: 30_000 });
  steps.push(`[verify]\n${formatRemoteResult(health)}`);

  if (waitForReady && head) {
    steps.push(`[wait for gateway] polling up to ${waitTimeoutSec}s for rev ${head.slice(0, 12)}…`);
    const wait = await waitForDeployReady(cfg, head, {
      timeoutMs: waitTimeoutSec * 1000,
    });
    steps.push(wait.log);
    if (!wait.ready) {
      steps.push(
        `[wait for gateway] TIMEOUT — Pi not ready after ${waitTimeoutSec}s (check marengo-gateway / www)`,
      );
    }
  }

  return steps.join("\n\n---\n\n");
}

/** Remote command: read canonical /opt/marengo/.deploy-rev (written by install-pi.sh). */
export function deployRevLogCommand(piRoot = "/opt/marengo"): string {
  return `cat ${shellQuote(`${piRoot}/.deploy-rev`)} 2>/dev/null || echo '(no .deploy-rev)'`;
}

async function logDeployRev(
  cfg: MarengoPiConfig,
  _runRemote: (body: string, timeoutMs?: number) => Promise<string>,
  steps: string[],
  expectedHead?: string,
  remote: typeof execRemote = execRemote,
): Promise<void> {
  const body = wrapRemote(cfg, deployRevLogCommand(cfg.piRoot));
  const r = await remote(cfg, body, { timeoutMs: 15_000 });
  steps.push(`[.deploy-rev]\n${formatRemoteResult(r)}`);
  if (expectedHead && r.stdout.trim()) {
    const first = r.stdout.trim().split(/\s+/)[0] ?? "";
    if (first && first !== expectedHead) {
      steps.push(
        `[.deploy-rev] warn: installed rev ${first} does not match deploy HEAD ${expectedHead}`,
      );
    }
  }
}
