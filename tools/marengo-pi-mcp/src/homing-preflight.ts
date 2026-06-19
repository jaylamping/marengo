/** Shared homing preflight helpers for MCP tools (mirrors scripts/homing-preflight.sh). */

/** motor-repl homing-status for the active MARENGO_CONFIG_DIR. */
export function homingStatusCommand(): string {
  return "bin/motor-repl homing-status";
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

/** Remote shell lines for pi_health homing section. */
export function homingHealthShell(): string {
  return [
    "echo",
    homingPreflightShell(false),
  ].join("\n");
}
