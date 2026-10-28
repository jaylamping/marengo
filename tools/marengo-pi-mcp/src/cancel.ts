/**
 * MCP cancellation (TOOL-03). Each tool call runs inside {@link callContext}, which
 * carries the client's abort signal. Once it fires no new remote call starts. For a
 * motion tool the running remote session is also ended: its process group gets
 * SIGTERM. marengo-pi runs under `timeout --foreground` ({@link piSessionTimeout}), so
 * it is in that group and runs its exit stop; motor-repl does too. Then motor-repl
 * disable runs as the sole CAN owner with the session's config dir (and the default
 * one), and marengo-pi.service is restarted when the session had stopped it. A
 * non-motion call (deploy, logs) in flight is left to finish, since killing
 * install-pi.sh midway leaves /opt half-installed.
 */

import { AsyncLocalStorage } from "node:async_hooks";
import { randomUUID } from "node:crypto";
import { soleCanOwnerShell } from "./can-owner.js";
import type { MarengoPiConfig } from "./config.js";
import { remoteConfigDir, shellQuote, wrapRemoteWithConfig } from "./env.js";
import { execRemote, formatRemoteResult } from "./ssh.js";

export interface CallContext {
  signal?: AbortSignal;
  /** The tool can move the arm: cancel kills its remote session and stops. */
  motion: boolean;
}

export const callContext = new AsyncLocalStorage<CallContext>();

export function callCancelled(): boolean {
  return callContext.getStore()?.signal?.aborted === true;
}

export const CANCELLED_TEXT = "Cancelled by the client: no further remote step was started.";

/** Unique argv tag for one remote `bash -s` so a cancel can find its process group. */
export function remoteSessionTag(): string {
  return `marengo-mcp-${randomUUID()}`;
}

/**
 * `timeout(1)` prefix for every marengo-pi pipe. Plain `timeout` calls setpgid and moves
 * itself and marengo-pi into a new process group, where a cancel's group SIGTERM never
 * reaches marengo-pi. `--foreground` keeps both in the session's group. The time cap is
 * unchanged: on expiry timeout still sends SIGTERM to marengo-pi and exits 124; it only
 * skips signalling marengo-pi's own children, and marengo-pi starts none.
 */
export function piSessionTimeout(seconds: number | string): string {
  return `timeout --foreground ${seconds}`;
}

/** The line `takeCanOwnershipShell` prints when the session stopped a running unit. */
const SESSION_STOPPED_UNIT = /^marengo-pi\.service restore after session: true$/m;

/**
 * Calls in which a remote step stopped a running marengo-pi.service. A multi-step tool
 * (the harness) stops the unit in one ssh and restores it in a last one, which a cancel
 * never starts, so the cancel stop restores it instead.
 */
const callStoppedUnit = new WeakSet<CallContext>();

export interface CancelStopOptions {
  /**
   * The cancelled session stopped a running marengo-pi.service. Killing its ssh kills that
   * shell by SIGPIPE, which skips its EXIT trap, so the cancel stop restores the unit.
   */
  restoreUnit?: boolean;
  /** Also disable this config dir's drives (the default config, when the session used another). */
  alsoDisableConfigDir?: string;
}

/**
 * Remote shell run over a fresh ssh after a motion call is cancelled: SIGTERM the
 * session's process group and wait for it, end any motor-repl, then disable every
 * drive as the sole CAN owner (which also ends any marengo-pi and restores the unit).
 * The disables run even when the session has already ended.
 */
export function cancelMotionSessionShell(tag: string, opts: CancelStopOptions = {}): string {
  const body = [
    ...(opts.restoreUnit
      ? [
          'echo "cancel: the session stopped marengo-pi.service; restoring it after the stop"',
          "MARENGO_PI_UNIT_RESTORE=true",
        ]
      : []),
    "_stop_rc=0",
    "bin/motor-repl disable || _stop_rc=$?",
    ...(opts.alsoDisableConfigDir
      ? [`MARENGO_CONFIG_DIR=${shellQuote(opts.alsoDisableConfigDir)} bin/motor-repl disable || _stop_rc=$?`]
      : []),
    '[[ "$_stop_rc" -eq 0 ]]',
  ];
  return [
    `_pid="$(pgrep -f -x 'bash -s ${tag}' | head -n 1 || true)"`,
    '_pgid=""',
    'if [[ -n "$_pid" ]]; then',
    // The session can end between pgrep and ps; strict mode must not end the stop here.
    '  _pgid="$(ps -o pgid= -p "$_pid" 2>/dev/null | tr -d " " || true)"',
    "fi",
    'if [[ -n "$_pgid" ]]; then',
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
    soleCanOwnerShell(body.join("\n")),
  ].join("\n");
}

/**
 * The `runRemote` every tool gets. After a cancel it starts nothing; a motion call's
 * running ssh is killed and {@link cancelMotionSessionShell} runs over a fresh ssh with
 * the session's config dir and the unit state the session reported before the kill.
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
    if (call && SESSION_STOPPED_UNIT.test(r.stdout)) callStoppedUnit.add(call);
    if (!r.cancelled) return formatRemoteResult(r);
    const sessionConfigDir = remoteConfigDir(body) ?? cfg.configDir;
    const shell = cancelMotionSessionShell(sessionTag, {
      restoreUnit: call !== undefined && callStoppedUnit.has(call),
      alsoDisableConfigDir: sessionConfigDir === cfg.configDir ? undefined : cfg.configDir,
    });
    const stop = await exec(cfg, wrapRemoteWithConfig(cfg, shell, sessionConfigDir), {
      timeoutMs: 60_000,
    });
    return `${formatRemoteResult(r)}\n\n--- cancel stop ---\n${formatRemoteResult(stop)}`;
  };
}
