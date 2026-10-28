import { create } from '@bufbuild/protobuf';
import { describe, expect, it } from 'vitest';

import {
  ActuatorLimitSnapshotSchema,
  JointActuatorLimitSchema,
} from '@/gen/marengo/v1/marengo_pb';
import { findSnapshotLimit, liveJointEnvelope } from '@/state/actuatorStore';

const snapshot = create(ActuatorLimitSnapshotSchema, {
  joints: [
    create(JointActuatorLimitSchema, {
      joint: 'right_lower_arm_yaw',
      posLowerRad: -1.2,
      posUpperRad: 1.4,
    }),
    create(JointActuatorLimitSchema, {
      joint: 'left_knee_pitch',
      posLowerRad: 0,
      posUpperRad: 2,
    }),
  ],
});

describe('findSnapshotLimit', () => {
  it('finds every joint the live snapshot carries, not only four bench joints (UI-10)', () => {
    expect(findSnapshotLimit(snapshot, 'right_lower_arm_yaw')?.posUpperRad).toBe(1.4);
    expect(liveJointEnvelope('left_knee_pitch', snapshot)).toMatchObject({
      hardLowerRad: 0,
      hardUpperRad: 2,
    });
  });

  it('returns null for a joint the snapshot does not carry', () => {
    expect(findSnapshotLimit(snapshot, 'right_elbow_pitch')).toBeNull();
    expect(findSnapshotLimit(null, 'right_lower_arm_yaw')).toBeNull();
  });
});
