/**
 * Bench SocketCAN has one owner at a time. Every motor-repl subcommand (even
 * status / homing-status / gravity-preview) builds a Davout supervisor, which
 * opens SocketCAN and transmits type-24 active-reporting frames to each motor
 * during construction. Beside a running marengo-pi that is a second writer on
 * the bus its fault authority watches; any CAN error frame latches a persistent
 * Transport fault in marengo-pi.
 *
 * The mcp251x controller has two RX buffers and reports an RX FIFO overrun as an
 * error frame (CAN_ERR_CRTL_RX_OVERFLOW, counted in `rx_over_errors`). A
 * motor-repl run ending immediately before marengo-pi starts saturates the bus
 * (its type-24 burst plus stop sequence, then marengo-pi's own type-24 burst)
 * while both processes churn the CPU, so marengo-pi's fresh socket can receive
 * that overrun: hence a settle window before every marengo-pi launch.
 */

import { shellQuote } from "./env.js";
import { INSTALLED_RESTART_HELPER } from "./tools/restart-marengo-pi.js";

/** `pgrep -x` names of processes that hold bench SocketCAN. */
const CAN_OWNER_PROCESSES = "marengo-pi|motor-repl";

/** Quiet window: CAN error counters must hold still this long before marengo-pi binds SocketCAN. */
export const CAN_SETTLE_WINDOW_SEC = 0.5;

/** Settle windows tried before refusing (counters still moving or a CAN owner remains). */
export const CAN_SETTLE_WINDOWS = 4;

/** Remote shell: poll (0.2 s steps, `tries` times) until no marengo-pi/motor-repl runs. Never fails. */
export function waitCanReleasedShell(tries = 15): string {
  return [
    `for _ in $(seq ${tries}); do`,
    `  pgrep -x '${CAN_OWNER_PROCESSES}' >/dev/null || break`,
    "  sleep 0.2",
    "done",
  ].join("\n");
}

/**
 * Remote shell run immediately before marengo-pi binds SocketCAN: wait for every
 * marengo-pi/motor-repl to exit, then require one CAN_SETTLE_WINDOW_SEC window with no
 * CAN owner and unchanged error counters (logs `can settle: ok … <counters>`). Error frames
 * reach only sockets open when they arrive, so the window keeps the previous owner's
 * traffic and teardown away from marengo-pi's fresh socket. When no window settles within
 * CAN_SETTLE_WINDOWS it logs `can settle: FAIL …` and runs `onUnsettled`.
 *
 * Also defines `can_error_counters` (usable by later lines in the same shell): one entry per
 * CAN netdev with sysfs `rx_errors`, `rx_over_errors`, `tx_errors` plus `ip -details` state,
 * bus errors, error-passive and bus-off; the driver emits an error frame whenever these move.
 * `MARENGO_CAN_SYSFS` overrides `/sys/class/net` (test seam).
 */
export function canSettleShell(onUnsettled: string): string {
  return [
    "can_error_counters() {",
    "  local _if _s _ip",
    "  for _if in can0 can1 can2; do",
    '    _s="${MARENGO_CAN_SYSFS:-/sys/class/net}/$_if/statistics"',
    '    [[ -d "$_s" ]] || continue',
    `    _ip="$(ip -details -statistics link show dev "$_if" 2>/dev/null | awk '/can state/ {s=$3} /re-started/ {getline; b=$2; p=$5; o=$6} END {printf "state=%s bus_errors=%s passive=%s bus_off=%s", s, b, p, o}')"`,
    `    printf '%s rx_errors=%s rx_over=%s tx_errors=%s %s; ' "$_if" "$(cat "$_s/rx_errors" 2>/dev/null)" "$(cat "$_s/rx_over_errors" 2>/dev/null)" "$(cat "$_s/tx_errors" 2>/dev/null)" "$_ip"`,
    "  done",
    "}",
    waitCanReleasedShell(),
    "CAN_SETTLE=unsettled",
    '_can_prev="$(can_error_counters)"',
    '_can_now="$_can_prev"',
    `for _ in $(seq ${CAN_SETTLE_WINDOWS}); do`,
    `  sleep ${CAN_SETTLE_WINDOW_SEC}`,
    '  _can_now="$(can_error_counters)"',
    `  if [[ "$_can_now" == "$_can_prev" ]] && ! pgrep -x '${CAN_OWNER_PROCESSES}' >/dev/null; then`,
    "    CAN_SETTLE=ok",
    "    break",
    "  fi",
    '  _can_prev="$_can_now"',
    "done",
    'if [[ "$CAN_SETTLE" == ok ]]; then',
    `  echo "can settle: ok (${CAN_SETTLE_WINDOW_SEC}s quiet, no CAN owner) $_can_now"`,
    "else",
    `  echo "can settle: FAIL (CAN error counters moving or a CAN owner remains after ${CAN_SETTLE_WINDOWS} windows) $_can_now" >&2`,
    onUnsettled,
    "fi",
  ].join("\n");
}

/** `onUnsettled` for canSettleShell inside a session group: refuse to start marengo-pi. */
export const REFUSE_UNSETTLED_MARENGO_PI = [
  'echo "refusing to start marengo-pi: it latches a persistent Transport fault on any CAN error frame" >&2',
  "exit 1",
].join("\n");

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
    waitCanReleasedShell(),
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
    canSettleShell(
      'echo "warning: CAN not settled; restarting marengo-pi.service anyway (its prior state)" >&2',
    ),
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
