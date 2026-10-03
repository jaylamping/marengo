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

/** Shell block: calibration record path + homing-status (warn-only). */
export function homingPreflightShell(strict = false): string {
  const strictEnv = strict ? "true" : "false";
  return [
    `export HOMING_PREFLIGHT_STRICT=${strictEnv}`,
    "./scripts/homing-preflight.sh",
  ].join("\n");
}

/** True when every reported joint is Verified (no Unhomed/Homing/Faulted). */
export function homingStatusOutputOk(output: string): boolean {
  if (output.includes("[exit ")) return false;
  if (/homing=(Unhomed|Homing|Faulted)/.test(output)) return false;
  if (!/homing=Verified/.test(output)) return false;
  return true;
}

/**
 * Warn-only homing report (pi_health, pi_sync_bench_config). Same CAN-owner
 * fallback as {@link homingStatusShell}; render with renderRobotStateHoming.
 */
export function homingReportShell(): string {
  return canOwnerBranch(
    homingPreflightShell(false),
    [canOwnedSkipLine("homing preflight"), robotStateSnapshotShell()].join("\n"),
  );
}
