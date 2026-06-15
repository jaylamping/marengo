import type { MarengoPiConfig } from "../config.js";
import { shellQuote, wrapRemote } from "../env.js";
import { execRemote, formatRemoteResult } from "../ssh.js";

const DEFAULT_WAIT_MS = 180_000;
const DEFAULT_INTERVAL_MS = 5_000;

function sleep(ms: number): Promise<void> {
  return new Promise((resolve) => setTimeout(resolve, ms));
}

/** Remote script: exit 0 when deploy rev matches prefix and gateway is serving. */
export function deployReadyCheckScript(expectedRevPrefix: string): string {
  const prefix = shellQuote(expectedRevPrefix);
  return [
    "set -euo pipefail",
    "ROOT=/opt/marengo",
    'REV="$(cat "${ROOT}/.deploy-rev" 2>/dev/null || echo missing)"',
    'echo "deploy-rev=${REV}"',
    `case "$REV" in ${prefix}*) ;; *) echo "rev mismatch (want prefix ${prefix})"; exit 10;; esac`,
    "systemctl is-active --quiet marengo-gateway || { echo 'marengo-gateway not active'; exit 11; }",
    'curl -sf "http://127.0.0.1:8080/health" >/dev/null || { echo "gateway /health failed"; exit 12; }',
    'test -f "${ROOT}/www/index.html" || { echo "missing ${ROOT}/www/index.html"; exit 13; }',
    "echo ready",
  ].join("\n");
}

export async function waitForDeployReady(
  cfg: MarengoPiConfig,
  expectedRevPrefix: string,
  opts: { timeoutMs?: number; intervalMs?: number } = {},
): Promise<{ ready: boolean; log: string }> {
  const timeoutMs = opts.timeoutMs ?? DEFAULT_WAIT_MS;
  const intervalMs = opts.intervalMs ?? DEFAULT_INTERVAL_MS;
  const deadline = Date.now() + timeoutMs;
  const body = wrapRemote(cfg, deployReadyCheckScript(expectedRevPrefix));
  const attempts: string[] = [];

  while (Date.now() < deadline) {
    const r = await execRemote(cfg, body, { timeoutMs: 30_000 });
    const chunk = `[wait ${new Date().toISOString()}]\n${formatRemoteResult(r)}`;
    attempts.push(chunk);
    if (r.exitCode === 0) {
      return { ready: true, log: attempts.join("\n\n") };
    }
    await sleep(intervalMs);
  }

  return { ready: false, log: attempts.join("\n\n") };
}
