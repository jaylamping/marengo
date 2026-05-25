import path from "node:path";
import type { MarengoPiConfig } from "../config.js";
import { shellQuote, wrapRemote } from "../env.js";
import { execLocal, execRemote, formatRemoteResult } from "../ssh.js";

export async function runSyncMain(
  cfg: MarengoPiConfig,
  runRemote: (body: string, timeoutMs?: number) => Promise<string>,
  strategy: "cross" | "pi_native",
): Promise<string> {
  const steps: string[] = [];

  if (strategy === "pi_native") {
    const body = wrapRemote(
      cfg,
      [
        "if ! git diff --quiet || ! git diff --cached --quiet; then git status --short; exit 1; fi",
        "git fetch origin && git checkout main && git pull --ff-only",
        "cargo build -p marengo-pi -p motor-repl --features socketcan --release",
        "sudo MARENGO_INSTALL_ROOT=/opt/marengo ./scripts/install-pi.sh",
      ].join("\n"),
    );
    const r = await execRemote(cfg, body, { timeoutMs: 900_000 });
    steps.push(formatRemoteResult(r));
    await writeDeployRev(cfg, runRemote, steps);
    return steps.join("\n\n---\n\n");
  }

  // cross-build from Mac
  const status = await execLocal("git", ["status", "--porcelain"], {
    cwd: cfg.localRoot,
  });
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

  const deployScript = path.join(cfg.localRoot, "scripts", "deploy-pi.sh");
  const deploy = await execLocal(
    "bash",
    [deployScript, `${cfg.user}@${cfg.host}`],
    { cwd: cfg.localRoot, timeoutMs: 900_000 },
  );
  steps.push(`[deploy-pi.sh]\n${formatRemoteResult(deploy)}`);
  if (deploy.exitCode !== 0) return steps.join("\n\n");

  const staging = cfg.piStagingRoot.replace(/^~/, `/home/${cfg.user}`);
  const installBody = [
    "set -euo pipefail",
    `cd ${shellQuote(staging)}`,
    "if pgrep -af marengo-pi >/dev/null 2>&1; then echo 'warning: marengo-pi running' >&2; fi",
    "sudo MARENGO_INSTALL_ROOT=/opt/marengo ./scripts/install-pi.sh",
  ].join("\n");
  const install = await execRemote(cfg, installBody, { timeoutMs: 120_000 });
  steps.push(`[install-pi.sh]\n${formatRemoteResult(install)}`);
  if (install.exitCode !== 0) return steps.join("\n\n");

  await writeDeployRev(cfg, runRemote, steps, head);

  const healthBody = wrapRemote(
    cfg,
    "cat .deploy-rev && test -x bin/marengo-pi && echo 'install ok'",
  );
  const health = await execRemote(cfg, healthBody, { timeoutMs: 30_000 });
  steps.push(`[verify]\n${formatRemoteResult(health)}`);

  return steps.join("\n\n---\n\n");
}

async function writeDeployRev(
  cfg: MarengoPiConfig,
  _runRemote: (body: string, timeoutMs?: number) => Promise<string>,
  steps: string[],
  headOverride?: string,
): Promise<void> {
  let head = headOverride;
  if (!head) {
    const rev = await execLocal("git", ["rev-parse", "HEAD"], {
      cwd: cfg.localRoot,
    });
    head = rev.stdout.trim();
  }
  const ts = new Date().toISOString();
  const content = `${head} ${ts}\n`;
  const body = wrapRemote(
    cfg,
    `echo ${JSON.stringify(content)} > .deploy-rev && cat .deploy-rev`,
  );
  const r = await execRemote(cfg, body, { timeoutMs: 15_000 });
  steps.push(`[.deploy-rev]\n${formatRemoteResult(r)}`);
}
