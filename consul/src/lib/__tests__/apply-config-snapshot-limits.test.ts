import { describe, expect, it } from 'vitest';

import { applyConfigSnapshotLimits } from '@/lib/apply-config-snapshot-limits';
import type { ConfigSnapshotDto } from '@/lib/config-api';

const base: ConfigSnapshotDto = {
  profile: 'master',
  config_dir: '/opt/marengo/config',
  joints: ['right_shoulder_pitch', 'right_elbow_pitch'],
  motors: [
    {
      joint: 'right_shoulder_pitch',
      can_interface: 'can0',
      device_id: 1,
      direction: -1,
      motor_type: 'rs03',
      bench: {
        position_lower_rad: -0.9,
        position_upper_rad: 2.9,
        torque_limit_nm: 5,
      },
    },
    {
      joint: 'right_elbow_pitch',
      can_interface: 'can0',
      device_id: 4,
      direction: 1,
      motor_type: 'rs02',
      bench: {
        position_lower_rad: -0.4,
        position_upper_rad: 1.0,
        torque_limit_nm: 3,
      },
    },
  ],
  control_limits: [
    {
      joint: 'right_shoulder_pitch',
      position_soft_lower_rad: -0.87,
      position_soft_upper_rad: 2.87,
    },
  ],
};

describe('applyConfigSnapshotLimits', () => {
  it('updates hard + soft for the applied joint only', () => {
    const next = applyConfigSnapshotLimits(base, 'right_shoulder_pitch', {
      lower: -0.53,
      upper: 1.23,
      softLower: -0.503,
      softUpper: 1.203,
    });
    expect(next).not.toBeNull();
    const pitch = next!.motors.find((m) => m.joint === 'right_shoulder_pitch');
    const elbow = next!.motors.find((m) => m.joint === 'right_elbow_pitch');
    expect(pitch?.bench.position_lower_rad).toBeCloseTo(-0.53, 6);
    expect(pitch?.bench.position_upper_rad).toBeCloseTo(1.23, 6);
    expect(elbow?.bench.position_upper_rad).toBeCloseTo(1.0, 6);
    const soft = next!.control_limits.find((c) => c.joint === 'right_shoulder_pitch');
    expect(soft?.position_soft_lower_rad).toBeCloseTo(-0.503, 6);
    expect(soft?.position_soft_upper_rad).toBeCloseTo(1.203, 6);
  });

  it('inserts soft row when control_limits lacked the joint', () => {
    const next = applyConfigSnapshotLimits(base, 'right_elbow_pitch', {
      lower: -0.5,
      upper: 0.95,
      softLower: -0.473,
      softUpper: 0.923,
    });
    const soft = next!.control_limits.find((c) => c.joint === 'right_elbow_pitch');
    expect(soft?.position_soft_lower_rad).toBeCloseTo(-0.473, 6);
    expect(soft?.position_soft_upper_rad).toBeCloseTo(0.923, 6);
  });

  it('returns null when snapshot is missing', () => {
    expect(
      applyConfigSnapshotLimits(null, 'right_shoulder_pitch', {
        lower: -1,
        upper: 1,
        softLower: -0.97,
        softUpper: 0.97,
      }),
    ).toBeNull();
  });
});
