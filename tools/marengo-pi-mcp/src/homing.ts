/**
 * Homing report for pi_health, pi_homing_status and pi_sync_bench_config. It never
 * opens CAN. Reference grants live only inside the marengo-pi process that acquired
 * them (ADR 0036), so a fresh motor-repl always reads Unhomed and carries no
 * information, while its Supervisor still opens SocketCAN and sends type-24 frames.
 */

import { robotStateSnapshotShell } from "./robot-state.js";

/** Printed when no marengo-pi runs: nothing on the Pi holds a reference grant. */
export const NO_LIVE_SESSION_LINE =
  "no live marengo-pi session: reference grants are process-local (ADR 0036)";

/** Read-only journal reader installed with the scripts tree (relative to MARENGO_ROOT). */
export const JOURNAL_TAIL_SCRIPT = "scripts/reference-journal-tail.py";

/**
 * Remote shell: while marengo-pi runs, its own per-joint homing from the gateway
 * RobotState snapshot (render with renderRobotStateHoming); otherwise
 * {@link NO_LIVE_SESSION_LINE} plus the latest reference journal rows, read with
 * SQLite `mode=ro`.
 */
export function homingReportShell(): string {
  return [
    "echo '=== homing ==='",
    "if pgrep -x marengo-pi >/dev/null; then",
    robotStateSnapshotShell(),
    "else",
    `echo '${NO_LIVE_SESSION_LINE}'`,
    `if [[ -f ${JOURNAL_TAIL_SCRIPT} ]]; then`,
    `python3 ${JOURNAL_TAIL_SCRIPT} || true`,
    "else",
    `echo 'reference journal: ${JOURNAL_TAIL_SCRIPT} not installed (pi_sync_main)'`,
    "fi",
    "fi",
  ].join("\n");
}
