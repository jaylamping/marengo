/**
 * Live per-joint state from marengo-pi without touching CAN: marengo-gateway
 * serves the last `marengo.v1.RobotState` marengo-pi published over Chappe at
 * `/snapshot/robot/state` (protobuf). Remote scripts print it base64 behind a
 * marker line; {@link renderRobotStateHoming} decodes that line locally.
 */

const SNAPSHOT_MARKER = "robot_state_b64=";

/** proto `JointHomingState` by number, spelled like motor-repl `homing=` output. */
const JOINT_HOMING_STATES = ["Unspecified", "Unhomed", "Homing", "Verified", "Faulted"];

export interface JointSnapshot {
  name: string;
  /** Joint-space position (rad). */
  position: number;
  /** Drive-reported joint-space torque (Nm). */
  effort: number;
  homing: string;
  driveActive: boolean;
}

export interface RobotStateSnapshot {
  timestampMs: number;
  joints: JointSnapshot[];
}

type ProtoField =
  | { field: number; wire: 0; value: number }
  | { field: number; wire: 1; value: number }
  | { field: number; wire: 2; value: Uint8Array };

/** proto3 wire walk yielding varint, double (fixed64) and length-delimited fields; fixed32 is skipped. */
function* protoFields(buf: Uint8Array): Generator<ProtoField> {
  let pos = 0;
  const varint = (): number => {
    let value = 0;
    for (let scale = 1; scale < 2 ** 64; scale *= 128) {
      if (pos >= buf.length) throw new Error("truncated varint");
      const byte = buf[pos++];
      value += (byte & 0x7f) * scale;
      if ((byte & 0x80) === 0) return value;
    }
    throw new Error("varint longer than 64 bits");
  };
  while (pos < buf.length) {
    const key = varint();
    const field = Math.floor(key / 8);
    const wire = key % 8;
    if (wire === 0) {
      yield { field, wire, value: varint() };
    } else if (wire === 2) {
      const length = varint();
      const end = pos + length;
      if (end > buf.length) throw new Error(`truncated field ${field}`);
      const value = buf.subarray(pos, end);
      pos = end;
      yield { field, wire, value };
    } else if (wire === 1) {
      if (pos + 8 > buf.length) throw new Error(`truncated field ${field}`);
      const value = new DataView(buf.buffer, buf.byteOffset + pos, 8).getFloat64(0, true);
      pos += 8;
      yield { field, wire, value };
    } else if (wire === 5) {
      pos += 4;
      if (pos > buf.length) throw new Error(`truncated field ${field}`);
    } else {
      throw new Error(`unsupported wire type ${wire} (field ${field})`);
    }
  }
}

function decodeJointState(buf: Uint8Array): JointSnapshot {
  const joint: JointSnapshot = {
    name: "",
    position: 0,
    effort: 0,
    homing: JOINT_HOMING_STATES[0],
    driveActive: false,
  };
  for (const f of protoFields(buf)) {
    if (f.field === 1 && f.wire === 2) joint.name = new TextDecoder().decode(f.value);
    else if (f.field === 2 && f.wire === 1) joint.position = f.value;
    else if (f.field === 4 && f.wire === 1) joint.effort = f.value;
    else if (f.field === 7 && f.wire === 0) {
      joint.homing = JOINT_HOMING_STATES[f.value] ?? `Unknown(${f.value})`;
    } else if (f.field === 8 && f.wire === 0) joint.driveActive = f.value !== 0;
  }
  return joint;
}

/** Decode `marengo.v1.RobotState` (timestamp_ms = 1, joints = 2) down to position/effort/homing fields. */
export function decodeRobotState(buf: Uint8Array): RobotStateSnapshot {
  const state: RobotStateSnapshot = { timestampMs: 0, joints: [] };
  for (const f of protoFields(buf)) {
    if (f.field === 1 && f.wire === 0) state.timestampMs = f.value;
    else if (f.field === 2 && f.wire === 2) state.joints.push(decodeJointState(f.value));
  }
  return state;
}

/** Remote shell: print the gateway RobotState snapshot as one base64 marker line. */
export function robotStateSnapshotShell(): string {
  return `printf '${SNAPSHOT_MARKER}'; curl -sf --max-time 3 http://127.0.0.1:8080/snapshot/robot/state | base64 -w0 || true; echo`;
}

/** Base64 payload of the first snapshot marker line in remote output; `undefined` when absent. */
export function robotStateMarkerPayload(output: string): string | undefined {
  return output.match(new RegExp(`^${SNAPSHOT_MARKER}(\\S*)$`, "m"))?.[1];
}

function formatRobotStateHoming(b64: string, nowMs: number): string {
  if (!b64) {
    return "marengo-pi RobotState: unavailable (gateway /snapshot/robot/state empty — gateway down or nothing published yet)";
  }
  let state: RobotStateSnapshot;
  try {
    state = decodeRobotState(Buffer.from(b64, "base64"));
  } catch (err) {
    return `marengo-pi RobotState: undecodable snapshot (${err instanceof Error ? err.message : String(err)})`;
  }
  const published =
    state.timestampMs > 0
      ? `published ${new Date(state.timestampMs).toISOString()}, ~${((nowMs - state.timestampMs) / 1000).toFixed(1)} s old by MCP host clock`
      : "timestamp unset";
  const joints = state.joints.length
    ? state.joints.map((j) => `${j.name}: homing=${j.homing} drive_active=${j.driveActive}`)
    : ["(no joints — marengo-pi publishes only joints with fresh CAN feedback)"];
  return [`marengo-pi RobotState via gateway (${published}):`, ...joints].join("\n");
}

/** Replace snapshot marker lines in remote output with decoded per-joint homing. */
export function renderRobotStateHoming(output: string, nowMs = Date.now()): string {
  return output.replace(
    new RegExp(`^${SNAPSHOT_MARKER}(\\S*)$`, "gm"),
    (_line, b64: string) => formatRobotStateHoming(b64, nowMs),
  );
}
