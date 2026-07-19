import { describe, expect, it } from 'vitest';

import { overlayNeedsCalibrationAck } from '@/lib/teach-calibration';
import {
  createTeachSession,
  overlayReplayAllowed,
} from '@/lib/teach-transit';

const joints = [
  'right_shoulder_pitch',
  'right_shoulder_roll',
  'right_upper_arm_yaw',
  'right_elbow_pitch',
];

describe('teach soft-invalidate calibration', () => {
  it('does not need ack when epochs match', () => {
    expect(
      overlayNeedsCalibrationAck(
        createTeachSession(
          { profile: 'arm_4dof_right', joints, deployRev: 'abc' },
          'wave',
          [],
          { calibrationEpoch: 2 }
        ),
        { liveCalibrationEpoch: 2, ackedAtEpoch: 2 }
      )
    ).toBe(false);
  });

  it('needs ack when live epoch advanced after set-zero mark', () => {
    const session = createTeachSession(
      { profile: 'arm_4dof_right', joints, deployRev: 'abc' },
      'wave',
      [],
      { calibrationEpoch: 1 }
    );
    expect(
      overlayNeedsCalibrationAck(session, {
        liveCalibrationEpoch: 2,
        ackedAtEpoch: 1,
      })
    ).toBe(true);
  });

  it('overlayReplayAllowed soft-blocks until ack', () => {
    const fp = { profile: 'arm_4dof_right', joints, deployRev: 'abc1234' };
    const session = createTeachSession(fp, 'wave', [], { calibrationEpoch: 0 });
    expect(
      overlayReplayAllowed(session, fp, {
        liveCalibrationEpoch: 1,
        ackedAtEpoch: 0,
      }).ok
    ).toBe(false);
    expect(
      overlayReplayAllowed(session, fp, {
        liveCalibrationEpoch: 1,
        ackedAtEpoch: 1,
      }).ok
    ).toBe(true);
  });
});
