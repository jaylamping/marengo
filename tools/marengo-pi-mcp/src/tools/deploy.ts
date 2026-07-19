import path from "node:path";
import type { MarengoPiConfig } from "../config.js";
import { sudoInstallCommand, sudoStagingInstallCommand } from "../config.js";
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
    const body = wrapRemote(
      cfg,
      [
        "if ! git diff --quiet || ! git diff --cached --quiet; then git status --short; exit 1; fi",
        "git fetch origin && git checkout main && git pull --ff-only",
        "# Bench: stop systemd marengo-pi before install replaces binary",
        "sudo systemctl stop marengo-pi.service 2>/dev/null || true",
        "sudo systemctl disable marengo-pi.service 2>/dev/null || true",
        "sudo pkill -f /opt/marengo/bin/marengo-pi 2>/dev/null || true",
        "sudo git config --global --add safe.directory \"$(pwd)\" 2>/dev/null || true",
        "if [[ -x ./scripts/pi-native-build.sh ]]; then",
        "  ./scripts/pi-native-build.sh",
        "else",
        '  if [[ -f "${HOME}/.cargo/env" ]]; then set -a; source "${HOME}/.cargo/env"; set +a; fi',
        '  export PATH="${HOME}/.cargo/bin:/usr/local/cargo/bin:${PATH:-}"',
        "  command -v cargo >/dev/null || { echo 'error: cargo not on PATH'; exit 127; }",
        "  cargo build -p marengo-pi -p marengo-gateway -p marengo-log-cli -p motor-repl -p imu-probe --features socketcan,linux-i2c --release",
        "  if [[ -x ./scripts/build-consul-native.sh ]]; then ./scripts/build-consul-native.sh; elif command -v npm >/dev/null && [[ -f consul/package-lock.json ]]; then",
        "    (cd consul && npm ci && env -u VITE_CHAPPE_HTTP_URL -u VITE_CHAPPE_WEBTRANSPORT_URL npm run build)",
        "  fi",
        "fi",
        sudoInstallCommand(cfg),
        "# Ensure deploy-rev + www even if install-pi.sh on Pi predates consul/dist + safe.directory fixes",
        'SHA="$(git rev-parse HEAD)"; TS="$(date -u +%Y-%m-%dT%H:%M:%SZ)"; printf "%s %s\\n" "$SHA" "$TS" | sudo tee /opt/marengo/.deploy-rev >/dev/null',
        "if [[ -f consul/dist/index.html ]]; then sudo rsync -a --delete consul/dist/ /opt/marengo/www/; fi",
        "sudo systemctl restart marengo-gateway.service 2>/dev/null || true",
      ].join("\n"),
    );
    const r = await execRemote(cfg, body, { timeoutMs: 900_000 });
    steps.push(formatRemoteResult(r));
    if (r.exitCode !== 0) return steps.join("\n\n---\n\n");

    const revBody = wrapRemote(cfg, "git rev-parse HEAD");
    const revR = await execRemote(cfg, revBody, { timeoutMs: 15_000 });
    const head = revR.stdout.trim();
    if (head) {
      steps.push(`[deploy rev] ${head}`);
    }
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

  // cross-build from local HEAD (current branch). Do NOT checkout main —
  // that steals the workspace off feature branches and deploys the wrong rev.
  const status = await execLocal(
    "git",
    ["status", "--porcelain", "--untracked-files=no"],
    { cwd: cfg.localRoot },
  );
  if (status.stdout.trim()) {
    return `Local repo dirty — commit or stash first:\n${status.stdout}`;
  }

  const branch = await execLocal(
    "git",
    ["rev-parse", "--abbrev-ref", "HEAD"],
    { cwd: cfg.localRoot },
  );
  const rev = await execLocal("git", ["rev-parse", "HEAD"], {
    cwd: cfg.localRoot,
  });
  const head = rev.stdout.trim();
  const branchName = branch.stdout.trim() || "(detached)";
  steps.push(`[local HEAD] ${branchName} ${head}`);
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
