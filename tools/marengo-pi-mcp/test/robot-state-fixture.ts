/** Test-only `marengo.v1.RobotState` protobuf encoder (wire format the gateway snapshot serves). */

function varint(n: number): number[] {
  const out: number[] = [];
  while (n >= 0x80) {
    out.push((n % 0x80) | 0x80);
    n = Math.floor(n / 0x80);
  }
  out.push(n);
  return out;
}

const tag = (field: number, wire: number) => varint(field * 8 + wire);

function lenField(field: number, bytes: number[]): number[] {
  return [...tag(field, 2), ...varint(bytes.length), ...bytes];
}

function doubleField(field: number, value: number): number[] {
  const bytes = Buffer.alloc(8);
  bytes.writeDoubleLE(value);
  return [...tag(field, 1), ...bytes];
}

export interface JointFixture {
  name: string;
  /** proto `JointHomingState` number (3 = Verified). */
  homing: number;
  driveActive: boolean;
  position?: number;
  effort?: number;
}

/** marengo.v1.JointState with every scalar the publisher sets (float exercises the fixed32 skip). */
function jointState(j: JointFixture): number[] {
  return [
    ...lenField(1, [...Buffer.from(j.name)]),
    ...doubleField(2, j.position ?? 0.25),
    ...doubleField(3, 0.01), // velocity
    ...doubleField(4, j.effort ?? -0.5),
    ...tag(5, 5), ...new Array(4).fill(0x22), // temperature_c (float)
    ...tag(6, 0), ...varint(0), // fault
    ...tag(7, 0), ...varint(j.homing),
    ...tag(8, 0), ...varint(j.driveActive ? 1 : 0),
  ];
}

export function encodeRobotState(timestampMs: number, joints: JointFixture[]): Uint8Array {
  return Uint8Array.from([
    ...tag(1, 0), ...varint(timestampMs),
    ...joints.flatMap((j) => lenField(2, jointState(j))),
  ]);
}
