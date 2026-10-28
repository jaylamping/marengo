/**
 * MCP cancellation (TOOL-03). Each tool call runs inside {@link callContext}, which
 * carries the client's abort signal. Once it fires no new remote call starts. For a
 * motion tool the running remote session is also ended: its process group gets
 * SIGTERM (marengo-pi and motor-repl run their exit stops), then motor-repl disable
 * runs as the sole CAN owner. A non-motion call (deploy, logs) in flight is left to
 * finish, since killing install-pi.sh midway leaves /opt half-installed.
 */

import { AsyncLocalStorage } from "node:async_hooks";
import { randomUUID } from "node:crypto";
import { soleCanOwnerShell } from "./can-owner.js";
import type { MarengoPiConfig } from "./config.js";
import { wrapRemote } from "./env.js";
import { execRemote, formatRemoteResult } from "./ssh.js";

export interface CallContext {
  signal?: AbortSignal;
  /** The tool can move the arm: cancel kills its remote session and stops. */
  motion: boolean;
  /** Last nonzero exit code a motion tool wrote to the audit log during this call. */
  auditedExitCode?: number;
}

export const callContext = new AsyncLocalStorage<CallContext>();

export function callCancelled(): boolean {
  return callContext.getStore()?.signal?.aborted === true;
}

/** Text prefixes that mark a refusal or failure for tools that do not audit. */
const ERROR_PREFIXES = ["Motion blocked", "Weighted motion blocked", "Error:", "Local repo dirty", "Refused"];

/**
 * MCP isError for a finished call: a refusal or failure prefix (including the management
 * gate's `<tool> refused:`), RECOVER_FAIL, or a motion tool that audited a nonzero exit
 * (gravity-gate refusals, refused sessions; TOOL-20).
 */
export function toolResultIsError(text: string, call: CallContext): boolean {
  return (
    ERROR_PREFIXES.some((p) => text.startsWith(p)) ||
    /^pi_[\w ]+ refused: /.test(text) ||
    text.includes("RECOVER_FAIL") ||
    (call.auditedExitCode ?? 0) !== 0
  );
}

export const CANCELLED_TEXT = "Cancelled by the client: no further remote step was started.";

/** Unique argv tag for one remote `bash -s` so a cancel can find its process group. */
export function remoteSessionTag(): string {
  return `marengo-mcp-${randomUUID()}`;
}

/**
 * Remote shell run over a fresh ssh after a motion call is cancelled: SIGTERM the
 * session's process group and wait for it, end any motor-repl, then disable every
 * drive as the sole CAN owner (which also ends any marengo-pi and restores the unit).
 */
export function cancelMotionSessionShell(tag: string): string {
  return [
    `_pid="$(pgrep -f -x 'bash -s ${tag}' | head -n 1 || true)"`,
    'if [[ -n "$_pid" ]]; then',
    '  _pgid="$(ps -o pgid= -p "$_pid" | tr -d " ")"',
    '  echo "cancel: ending remote session $_pid (group $_pgid)"',
    '  kill -TERM -- "-$_pgid" 2>/dev/null || true',
    "  for _ in $(seq 50); do",
    '    pgrep -g "$_pgid" >/dev/null || break',
    "    sleep 0.2",
    "  done",
    "else",
    '  echo "cancel: remote session already ended"',
    "fi",
    "pkill -x motor-repl 2>/dev/null || true",
    soleCanOwnerShell("bin/motor-repl disable"),
  ].join("\n");
}

/**
 * The `runRemote` every tool gets. After a cancel it starts nothing; a motion call's
 * running ssh is killed and {@link cancelMotionSessionShell} runs over a fresh ssh.
 */
export function makeRunRemote(cfg: MarengoPiConfig, exec: typeof execRemote = execRemote) {
  return async (body: string, timeoutMs?: number): Promise<string> => {
    if (callCancelled()) return `${CANCELLED_TEXT}\n\n[exit 130]`;
    const call = callContext.getStore();
    const sessionTag = remoteSessionTag();
    const r = await exec(cfg, body, {
      timeoutMs,
      sessionTag,
      signal: call?.motion ? call.signal : undefined,
    });
    if (!r.cancelled) return formatRemoteResult(r);
    const stop = await exec(cfg, wrapRemote(cfg, cancelMotionSessionShell(sessionTag)), {
      timeoutMs: 60_000,
    });
    return `${formatRemoteResult(r)}\n\n--- cancel stop ---\n${formatRemoteResult(stop)}`;
  };
}
