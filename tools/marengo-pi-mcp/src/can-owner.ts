/**
 * Bench SocketCAN has one owner at a time. Every motor-repl subcommand (even
 * status / homing-status / gravity-preview) builds a Davout supervisor, which
 * opens SocketCAN and transmits type-24 active-reporting frames to each motor
 * during construction. Beside a running marengo-pi that is a second writer on
 * the bus its fault authority watches; any CAN error frame latches a persistent
 * Transport fault in marengo-pi.
 */

import { shellQuote } from "./env.js";

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
