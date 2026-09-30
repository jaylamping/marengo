import path from "node:path";
import type { MarengoPiConfig } from "../config.js";
import { sudoStagingInstallCommand } from "../config.js";
import { shellQuote, wrapRemote } from "../env.js";
import { execLocal, execRemote, formatRemoteResult } from "../ssh.js";
import { waitForDeployReady } from "./deploy-wait.js";

function localDeployCommand(cfg: MarengoPiConfig): { cmd: string; args: string[] } {
  const host = `${cfg.user}@${cfg.host}`;
  if (process.platform === "win32") {
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
        host,
      ],
    };
  }
  return {
    cmd: "bash",
    args: ["./scripts/deploy-pi.sh", "--install", host],
  };
}

export async function runSyncMain(
  cfg: MarengoPiConfig,
  runRemote: (body: string, timeoutMs?: number) => Promise<string>,
  strategy: "cross" | "pi_native",
  opts: { waitForReady?: boolean; waitTimeoutSec?: number } = {},
): Promise<string> {
  const waitForReady = opts.waitForReady ?? true;
  const waitTimeoutSec = opts.waitTimeoutSec ?? 180;
  const steps: string[] = [];

  if (strategy === "pi_native") {
    // Canonical path: scripts/pi-self-update.sh (shared with Consul /control/deploy).
    const tipBody = wrapRemote(
      cfg,
      [
        "git fetch origin",
        "git rev-parse origin/main",
      ].join("\n"),
    );
    const tipR = await execRemote(cfg, tipBody, { timeoutMs: 120_000 });
    steps.push(`[resolve origin/main]\n${formatRemoteResult(tipR)}`);
    if (tipR.exitCode !== 0) return steps.join("\n\n---\n\n");
    const head = tipR.stdout.trim().split(/\s+/).pop() ?? "";
    if (!/^[0-9a-f]{7,40}$/i.test(head)) {
      steps.push(`[resolve origin/main] error: bad SHA '${head}'`);
      return steps.join("\n\n---\n\n");
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
    const r = await execRemote(cfg, body, { timeoutMs: 900_000 });
    steps.push(formatRemoteResult(r));
    if (r.exitCode !== 0) return steps.join("\n\n---\n\n");

    await logDeployRev(cfg, runRemote, steps, head || undefined);

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

  // cross-build from Mac
  const status = await execLocal(
    "git",
    ["status", "--porcelain", "--untracked-files=no"],
    { cwd: cfg.localRoot },
  );
  if (status.stdout.trim()) {
    return `Local repo dirty — commit or stash first:\n${status.stdout}`;
  }

  const fetch = await execLocal(
    "git",
    ["fetch", "origin", "main"],
    { cwd: cfg.localRoot, timeoutMs: 120_000 },
  );
  steps.push(`[local git fetch]\n${formatRemoteResult(fetch)}`);
  if (fetch.exitCode !== 0) return steps.join("\n\n");

  const checkout = await execLocal(
    "git",
    ["checkout", "main"],
    { cwd: cfg.localRoot },
  );
  steps.push(`[local checkout main]\n${formatRemoteResult(checkout)}`);
  if (checkout.exitCode !== 0) return steps.join("\n\n");

  const pull = await execLocal(
    "git",
    ["pull", "--ff-only", "origin", "main"],
    { cwd: cfg.localRoot, timeoutMs: 120_000 },
  );
  steps.push(`[local pull]\n${formatRemoteResult(pull)}`);
  if (pull.exitCode !== 0) return steps.join("\n\n");

  const rev = await execLocal("git", ["rev-parse", "HEAD"], {
    cwd: cfg.localRoot,
  });
  const head = rev.stdout.trim();
  steps.push(`[deploy rev] ${head}`);

  const deploy = localDeployCommand(cfg);
  const deployResult = await execLocal(deploy.cmd, deploy.args, {
    cwd: cfg.localRoot,
    timeoutMs: 900_000,
  });
  steps.push(
    `[${path.basename(deploy.args[0])}]\n${formatRemoteResult(deployResult)}`,
  );
  if (deployResult.exitCode !== 0) return steps.join("\n\n");

  const installBody = wrapRemote(cfg, sudoStagingInstallCommand(cfg));
  const installOpt = await execRemote(cfg, installBody, { timeoutMs: 120_000 });
  steps.push(`[install-pi.sh staging → /opt]\n${formatRemoteResult(installOpt)}`);
  if (installOpt.exitCode !== 0) return steps.join("\n\n");

  const staging = cfg.piStagingRoot.replace(/^~/, `/home/${cfg.user}`);
  const verifyBody = [
    "set -euo pipefail",
    `cd ${shellQuote(staging)}`,
    "if pgrep -af marengo-pi >/dev/null 2>&1; then echo 'warning: marengo-pi running' >&2; fi",
    "test -x /opt/marengo/bin/marengo-pi && /opt/marengo/bin/marengo-pi 2>&1 | head -1 || true",
    "echo 'install verified'",
  ].join("\n");
  const verify = await execRemote(cfg, verifyBody, { timeoutMs: 120_000 });
  steps.push(`[verify install]\n${formatRemoteResult(verify)}`);
  if (verify.exitCode !== 0) return steps.join("\n\n");

  await logDeployRev(cfg, runRemote, steps, head);

  const healthBody = wrapRemote(
    cfg,
    "test -x bin/marengo-pi && echo 'install ok'",
  );
  const health = await execRemote(cfg, healthBody, { timeoutMs: 30_000 });
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
): Promise<void> {
  const body = wrapRemote(cfg, deployRevLogCommand(cfg.piRoot));
  const r = await execRemote(cfg, body, { timeoutMs: 15_000 });
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
