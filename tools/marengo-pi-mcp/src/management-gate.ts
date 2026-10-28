/**
 * Deploy, install, build and restart all stop marengo-pi. The gateway refuses the same
 * operations from Consul unless the runtime mode is known, not Active, and its heartbeat
 * is fresh (`marengo-gateway/src/restart.rs` `refuse_unsafe_management_state`); the MCP
 * applies the same rule (P2B-05). When marengo-pi is not running nothing holds the arm,
 * so the operation is allowed. A running motor-repl is a CAN session in progress and
 * always refuses. Stops (`pi_hold_off`, `pi_motor_disable`, restart `mode=stop`) are
 * never gated.
 */

import type { MarengoPiConfig } from "./config.js";
import { wrapRemote } from "./env.js";
import { protoFields } from "./robot-state.js";

/** Same window as the gateway's `HEARTBEAT_FRESH_MS`. */
export const MANAGEMENT_HEARTBEAT_FRESH_MS = 5_000;

const MARKER = {
  marengoPi: "mgmt_marengo_pi=",
  motorRepl: "mgmt_motor_repl=",
  safety: "mgmt_safety_b64=",
  heartbeat: "mgmt_heartbeat_b64=",
  now: "mgmt_now_ms=",
} as const;

/** proto `OperationalMode`. */
const MODE_NAMES = ["Unspecified", "Disabled", "Ready", "Active"];
const MODE_DISABLED = 1;
const MODE_READY = 2;

/** Remote shell: process counts, the gateway safety and heartbeat snapshots, then the Pi clock. */
export function managementGateShell(): string {
  const snapshot = (marker: string, topic: string) =>
    `printf '${marker}'; curl -sf --max-time 3 http://127.0.0.1:8080/snapshot/robot/${topic} | base64 -w0 || true; echo`;
  return [
    `printf '${MARKER.marengoPi}%s\\n' "$(pgrep -xc marengo-pi || true)"`,
    `printf '${MARKER.motorRepl}%s\\n' "$(pgrep -xc motor-repl || true)"`,
    snapshot(MARKER.safety, "safety"),
    snapshot(MARKER.heartbeat, "heartbeat"),
    // Read last so a heartbeat published during the curls is never in the future.
    `printf '${MARKER.now}%s\\n' "$(date +%s%3N)"`,
  ].join("\n");
}

function markerValue(output: string, marker: string): string | undefined {
  return output.match(new RegExp(`^${marker}(\\S*)$`, "m"))?.[1];
}

function varintField(b64: string | undefined, field: number): number | undefined {
  if (!b64) return undefined;
  try {
    for (const f of protoFields(Buffer.from(b64, "base64"))) {
      if (f.field === field && f.wire === 0) return f.value;
    }
  } catch {
    return undefined;
  }
  return undefined;
}

/** Why the operation is refused, or `undefined` when it may run. */
export function managementRefusal(output: string, action: string): string | undefined {
  const refuse = (why: string) =>
    `${action} refused: ${why}. Stop control first with pi_hold_off (support the arm), ` +
    "then retry; the deploy leaves marengo-pi stopped.";
  const marengoPi = Number(markerValue(output, MARKER.marengoPi));
  const motorRepl = Number(markerValue(output, MARKER.motorRepl));
  if (!Number.isInteger(marengoPi) || !Number.isInteger(motorRepl)) {
    return refuse("could not read the Pi process table");
  }
  if (motorRepl > 0) return refuse("a motor-repl CAN session is running");
  if (marengoPi === 0) return undefined;
  // proto3 omits a zero varint, so an empty but present snapshot reads as Unspecified.
  const safety = markerValue(output, MARKER.safety);
  const mode = safety ? (varintField(safety, 2) ?? 0) : undefined;
  if (mode === undefined) return refuse("marengo-pi is running but its mode is unknown (no gateway safety snapshot)");
  if (mode !== MODE_DISABLED && mode !== MODE_READY) {
    return refuse(`marengo-pi is running in mode ${MODE_NAMES[mode] ?? `Unknown(${mode})`}`);
  }
  const heartbeatMs = varintField(markerValue(output, MARKER.heartbeat), 1);
  const nowMs = Number(markerValue(output, MARKER.now));
  if (heartbeatMs === undefined || !Number.isFinite(nowMs)) {
    return refuse("marengo-pi is running but has no heartbeat");
  }
  const ageMs = nowMs - heartbeatMs;
  if (ageMs < 0 || ageMs > MANAGEMENT_HEARTBEAT_FRESH_MS) {
    return refuse(`marengo-pi heartbeat is stale (age ${ageMs} ms)`);
  }
  return undefined;
}

/** Read the Pi and return the refusal text, or `undefined` when `action` may run. */
export async function checkManagementGate(
  cfg: MarengoPiConfig,
  runRemote: (body: string, timeoutMs?: number) => Promise<string>,
  action: string,
): Promise<string | undefined> {
  return managementRefusal(await runRemote(wrapRemote(cfg, managementGateShell()), 20_000), action);
}
