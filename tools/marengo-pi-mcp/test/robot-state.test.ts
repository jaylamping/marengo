import { describe, it } from "node:test";
import assert from "node:assert/strict";
import {
  decodeRobotStateHoming,
  renderRobotStateHoming,
  robotStateSnapshotShell,
} from "../src/robot-state.js";

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
const lenField = (field: number, bytes: number[]) => [...tag(field, 2), ...varint(bytes.length), ...bytes];

/** marengo.v1.JointState with every scalar the publisher sets (doubles/float exercise skips). */
function jointState(name: string, homing: number, driveActive: boolean): number[] {
  return [
    ...lenField(1, [...Buffer.from(name)]),
    ...tag(2, 1), ...new Array(8).fill(0x11), // position (double)
    ...tag(5, 5), ...new Array(4).fill(0x22), // temperature_c (float)
    ...tag(6, 0), ...varint(0), // fault
    ...tag(7, 0), ...varint(homing),
    ...tag(8, 0), ...varint(driveActive ? 1 : 0),
  ];
}

const TIMESTAMP_MS = 1_790_000_000_123;
const SNAPSHOT = Uint8Array.from([
  ...tag(1, 0), ...varint(TIMESTAMP_MS),
  ...lenField(2, jointState("right_shoulder_pitch", 3, true)),
  ...lenField(2, jointState("right_elbow_pitch", 1, false)),
]);

describe("RobotState homing snapshot", () => {
  it("decodes timestamp, joint names, homing state and drive_active", () => {
    assert.deepEqual(decodeRobotStateHoming(SNAPSHOT), {
      timestampMs: TIMESTAMP_MS,
      joints: [
        { name: "right_shoulder_pitch", homing: "Verified", driveActive: true },
        { name: "right_elbow_pitch", homing: "Unhomed", driveActive: false },
      ],
    });
  });

  it("rejects truncated snapshots", () => {
    assert.throws(() => decodeRobotStateHoming(SNAPSHOT.subarray(0, SNAPSHOT.length - 3)));
  });

  it("renders the marker line as motor-repl style homing= lines", () => {
    const b64 = Buffer.from(SNAPSHOT).toString("base64");
    const out = renderRobotStateHoming(`before\nrobot_state_b64=${b64}\nafter`, TIMESTAMP_MS + 1500);
    assert.match(out, /^before\nmarengo-pi RobotState via gateway \(published .*~1\.5 s old/);
    assert.match(out, /right_shoulder_pitch: homing=Verified drive_active=true/);
    assert.match(out, /right_elbow_pitch: homing=Unhomed drive_active=false\nafter$/);
  });

  it("reports an empty snapshot as unavailable", () => {
    assert.match(renderRobotStateHoming("robot_state_b64=\n"), /RobotState: unavailable/);
  });

  it("reads the gateway snapshot over loopback HTTP only", () => {
    assert.match(robotStateSnapshotShell(), /curl -sf .*http:\/\/127\.0\.0\.1:8080\/snapshot\/robot\/state \| base64 -w0/);
  });
});
