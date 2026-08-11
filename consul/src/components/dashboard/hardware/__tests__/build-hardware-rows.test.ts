import { describe, expect, it } from 'vitest';

import {
  buildHardwareRows,
  compareHardwareRowsByCanAddress,
} from '@/components/dashboard/hardware/build-hardware-rows';
import type { ConfigSnapshotDto } from '@/lib/config-api';

const snapshot: ConfigSnapshotDto = {
  profile: 'master',
  config_dir: '/opt/marengo/config',
  joints: [
    'right_shoulder_roll',
    'right_shoulder_pitch',
    'right_upper_arm_yaw',
    'right_elbow_pitch',
    'right_lower_arm_yaw',
  ],
  motors: [
    {
      joint: 'right_shoulder_roll',
      can_interface: 'can0',
      device_id: 2,
      direction: 1,
      motor_type: 'rs03',
      bench: {
        position_lower_rad: -0.05,
        position_upper_rad: 2.5,
        torque_limit_nm: 5,
      },
    },
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
      joint: 'right_upper_arm_yaw',
      can_interface: 'can0',
      device_id: 3,
      direction: 1,
      motor_type: 'rs02',
      bench: {
        position_lower_rad: -0.7,
        position_upper_rad: 0.5,
        torque_limit_nm: 3,
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
    {
      joint: 'right_lower_arm_yaw',
      can_interface: 'can0',
      device_id: 5,
      direction: 1,
      motor_type: 'rs00',
      bench: {
        position_lower_rad: -1.6,
        position_upper_rad: 1.6,
        torque_limit_nm: 1,
      },
    },
  ],
  control_limits: [],
};

describe('buildHardwareRows', () => {
  it('orders rows by CAN device id, not robot.joints order', () => {
    const rows = buildHardwareRows(snapshot, [], null, null);
    expect(rows.map((r) => r.canId)).toEqual([1, 2, 3, 4, 5]);
    expect(rows.map((r) => r.joint)).toEqual([
      'right_shoulder_pitch',
      'right_shoulder_roll',
      'right_upper_arm_yaw',
      'right_elbow_pitch',
      'right_lower_arm_yaw',
    ]);
  });
});

describe('compareHardwareRowsByCanAddress', () => {
  it('sorts by interface then id, with null ids last', () => {
    const rows = [
      { joint: 'b', canInterface: 'can0', canId: 2 },
      { joint: 'offline', canInterface: null, canId: null },
      { joint: 'a', canInterface: 'can0', canId: 1 },
      { joint: 'can1-first', canInterface: 'can1', canId: 1 },
    ];
    rows.sort(compareHardwareRowsByCanAddress);
    expect(rows.map((r) => r.joint)).toEqual([
      'a',
      'b',
      'can1-first',
      'offline',
    ]);
  });
});
