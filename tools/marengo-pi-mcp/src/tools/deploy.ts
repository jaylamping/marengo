import path from "node:path";
import { homedir } from "node:os";
import type { MarengoPiConfig } from "../config.js";
import { sudoInstallCommand, sudoStagingInstallCommand } from "../config.js";
import { shellQuote, wrapRemote } from "../env.js";
import { execLocal, execRemote, formatRemoteResult } from "../ssh.js";
import { waitForDeployReady } from "./deploy-wait.js";

function localDeployCommand(cfg: MarengoPiConfig): { cmd: string; args: string[] } {
  const host = `${cfg.user}@${cfg.host}`;
  // Relative paths: execLocal sets cwd to localRoot (Windows bash mangles C:\… backslashes).
  if (process.platform === "win32") {
    const sshDir = path
      .join(process.env.USERPROFILE ?? homedir(), ".ssh")
      .replace(/\\/g, "/");
    return {
      cmd: "bash",
      args: [
        "-c",
        `export MSYS_NO_PATHCONV=1; export MARENGO_SSH_DIR='${sshDir.replace(/'/g, `'\\''`)}'; export MARENGO_PI_HOST='${cfg.host.replace(/'/g, `'\\''`)}'; export MARENGO_PI_USER='${cfg.user.replace(/'/g, `'\\''`)}'; exec ./scripts/deploy-pi-docker.sh '${host.replace(/'/g, `'\\''`)}'`,
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
    const body = wrapRemote(
      cfg,
      [
        "if ! git diff --quiet || ! git diff --cached --quiet; then git status --short; exit 1; fi",
        "git fetch origin && git checkout main && git pull --ff-only",
        'if [[ -f "${HOME}/.cargo/env" ]]; then set -a; source "${HOME}/.cargo/env"; set +a; fi',
        'export PATH="${HOME}/.cargo/bin:/usr/local/cargo/bin:${PATH:-}"',
        "command -v cargo >/dev/null || { echo 'error: cargo not on PATH (install Rust on Pi or use pi_sync_main cross)'; exit 127; }",
        "cargo build -p marengo-pi -p motor-repl --features socketcan,linux-i2c --release",
        sudoInstallCommand(cfg),
      ].join("\n"),
    );
    const r = await execRemote(cfg, body, { timeoutMs: 900_000 });
    steps.push(formatRemoteResult(r));
    if (r.exitCode === 0) {
      await logDeployRev(cfg, runRemote, steps);
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
