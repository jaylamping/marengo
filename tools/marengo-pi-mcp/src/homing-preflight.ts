/** Shared homing preflight helpers for MCP tools (mirrors scripts/homing-preflight.sh). */

import { canOwnedSkipLine, canOwnerBranch } from "./can-owner.js";
import { robotStateSnapshotShell } from "./robot-state.js";

/**
 * Remote shell: per-joint homing readback that never opens CAN beside a running
 * owner — then it prints marengo-pi's own homing from the gateway snapshot
 * (render the output with renderRobotStateHoming).
 */
export function homingStatusShell(): string {
  return canOwnerBranch(
    "bin/motor-repl homing-status",
    [canOwnedSkipLine("motor-repl homing-status"), robotStateSnapshotShell()].join("\n"),
  );
}

/**
 * Warn-only homing report (pi_health, pi_sync_bench_config): calibration record path +
 * homing-status via scripts/homing-preflight.sh. Same CAN-owner fallback as
 * {@link homingStatusShell}; render with renderRobotStateHoming. A fresh motor-repl
 * holds no reference grant, so its joints read Unhomed; the gateway snapshot shows
 * marengo-pi's in-process grants.
 */
export function homingReportShell(): string {
  return canOwnerBranch(
    "export HOMING_PREFLIGHT_STRICT=false\n./scripts/homing-preflight.sh",
    [canOwnedSkipLine("homing preflight"), robotStateSnapshotShell()].join("\n"),
  );
}
