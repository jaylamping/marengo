/**
 * Bench SocketCAN has one owner at a time. Every motor-repl subcommand (even
 * status / homing-status / gravity-preview) builds a Davout supervisor, which
 * opens SocketCAN and transmits type-24 active-reporting frames to each motor
 * during construction. Beside a running marengo-pi that is a second writer on
 * the bus its fault authority watches; any CAN error frame latches a persistent
 * Transport fault in marengo-pi.
 */

import { shellQuote } from "./env.js";
import { INSTALLED_RESTART_HELPER } from "./tools/restart-marengo-pi.js";

/** `pgrep -x` names of processes that hold bench SocketCAN. */
const CAN_OWNER_PROCESSES = "marengo-pi|motor-repl";

/**
 * Remote shell: run `free` when no CAN owner is running, else `owned`.
 * Both branches see `CAN_OWNER_PID` / `CAN_OWNER_NAME` (empty when free).
 */
export function canOwnerBranch(free: string, owned: string): string {
  return [
    `CAN_OWNER="$(pgrep -l -x '${CAN_OWNER_PROCESSES}' | head -n 1 || true)"`,
    'CAN_OWNER_PID="${CAN_OWNER%% *}"',
    'CAN_OWNER_NAME="${CAN_OWNER#* }"',
    'if [[ -n "$CAN_OWNER" ]]; then',
    owned,
    "else",
    free,
    "fi",
  ].join("\n");
}

/** Remote shell line explaining why a CAN-opening step did not run. */
export function canOwnedSkipLine(what: string): string {
  return `printf '%s skipped: %s (pid %s) owns CAN\\n' ${shellQuote(what)} "$CAN_OWNER_NAME" "$CAN_OWNER_PID"`;
}

/** Remote shell: run a read-only CAN-opening command only when the bus is free. */
export function unlessCanOwned(command: string): string {
  return canOwnerBranch(command, canOwnedSkipLine(command));
}

/**
 * Remote shell: make this session the sole CAN owner. marengo-pi.service runs
 * as another user with Restart=always, so a bare pkill either fails or lets
 * systemd respawn a second owner mid-session; stop it through the
 * pi_restart_marengo_pi helper (passwordless sudo), kill this user's leftover
 * marengo-pi, then exit 1 unless no marengo-pi/motor-repl remains.
 * Sets MARENGO_PI_UNIT_RESTORE=true when the unit was running.
 */
export function takeCanOwnershipShell(): string {
  const helper = shellQuote(INSTALLED_RESTART_HELPER);
  return [
    "MARENGO_PI_UNIT_RESTORE=false",
    'case "$(systemctl is-active marengo-pi.service 2>/dev/null || true)" in',
    "  active|activating|reloading) MARENGO_PI_UNIT_RESTORE=true ;;",
    "esac",
    'echo "marengo-pi.service restore after session: $MARENGO_PI_UNIT_RESTORE"',
    `sudo -n ${helper} stop || echo "warning: sudo -n ${INSTALLED_RESTART_HELPER} stop failed" >&2`,
    "pkill -x marengo-pi 2>/dev/null || true",
    "for _ in $(seq 15); do",
    `  pgrep -x '${CAN_OWNER_PROCESSES}' >/dev/null || break`,
    "  sleep 0.2",
    "done",
    canOwnerBranch(
      ":",
      [
        `printf 'error: %s (pid %s) still owns CAN; refusing to open a second owner\\n' "$CAN_OWNER_NAME" "$CAN_OWNER_PID" >&2`,
        "exit 1",
      ].join("\n"),
    ),
  ].join("\n");
}

/** Remote shell: restart marengo-pi.service (starts Disabled) when the take step found it running. */
export function restoreCanOwnerShell(): string {
  return [
    'if [[ "${MARENGO_PI_UNIT_RESTORE:-false}" == true ]]; then',
    "  MARENGO_PI_UNIT_RESTORE=false",
    "  pkill -x marengo-pi 2>/dev/null || true",
    "  for _ in $(seq 15); do",
    `    pgrep -x '${CAN_OWNER_PROCESSES}' >/dev/null || break`,
    "    sleep 0.2",
    "  done",
    '  echo "=== restoring marengo-pi.service (active before this session) ==="',
    `  sudo -n ${shellQuote(INSTALLED_RESTART_HELPER)} restart || echo "warning: marengo-pi.service not restored; run pi_restart_marengo_pi" >&2`,
    "fi",
  ].join("\n");
}

/**
 * Remote shell: run `body` as the sole CAN owner and restore marengo-pi.service's
 * prior state on every exit path (normal exit, `set -e`, refusal, HUP/INT/TERM),
 * mirroring install-pi.sh's preserve-state behavior.
 */
export function soleCanOwnerShell(body: string): string {
  return [
    "MARENGO_PI_UNIT_RESTORE=false",
    "restore_can_owner() {",
    "  trap - EXIT HUP INT TERM",
    restoreCanOwnerShell(),
    "}",
    "trap restore_can_owner EXIT",
    "trap 'exit 129' HUP",
    "trap 'exit 130' INT",
    "trap 'exit 143' TERM",
    takeCanOwnershipShell(),
    body,
  ].join("\n");
}
