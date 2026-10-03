import { describe, it } from "node:test";
import assert from "node:assert/strict";
import {
  decodeRobotState,
  renderRobotStateHoming,
  robotStateMarkerPayload,
  robotStateSnapshotShell,
} from "../src/robot-state.js";
import { encodeRobotState } from "./robot-state-fixture.js";

const TIMESTAMP_MS = 1_790_000_000_123;
const SNAPSHOT = encodeRobotState(TIMESTAMP_MS, [
  { name: "right_shoulder_pitch", homing: 3, driveActive: true },
  { name: "right_elbow_pitch", homing: 1, driveActive: false },
]);

describe("RobotState homing snapshot", () => {
  it("decodes timestamp, joint names, position, effort, homing state and drive_active", () => {
    assert.deepEqual(decodeRobotState(SNAPSHOT), {
      timestampMs: TIMESTAMP_MS,
      joints: [
        {
          name: "right_shoulder_pitch",
          position: 0.25,
          effort: -0.5,
          homing: "Verified",
          driveActive: true,
        },
        {
          name: "right_elbow_pitch",
          position: 0.25,
          effort: -0.5,
          homing: "Unhomed",
          driveActive: false,
        },
      ],
    });
  });

  it("rejects truncated snapshots", () => {
    assert.throws(() => decodeRobotState(SNAPSHOT.subarray(0, SNAPSHOT.length - 3)));
  });

  it("extracts the marker payload from remote output", () => {
    assert.equal(robotStateMarkerPayload("a\nrobot_state_b64=QUJD\nb"), "QUJD");
    assert.equal(robotStateMarkerPayload("robot_state_b64=\n"), "");
    assert.equal(robotStateMarkerPayload("no marker"), undefined);
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
