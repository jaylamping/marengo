import { describe, expect, it } from 'vitest';

import { enrichInventory } from '@/lib/enrich-inventory';
import type { ConfigSnapshotDto } from '@/lib/config-api';
import { robotInventory } from '@/data/robot-inventory';

const snapshot: ConfigSnapshotDto = {
  profile: 'arm_2dof_right',
  config_dir: '/opt/marengo/config',
  joints: ['right_shoulder_roll', 'right_shoulder_pitch'],
  motors: [
    {
      joint: 'right_shoulder_roll',
      can_interface: 'can0',
      device_id: 1,
      direction: 1,
      motor_type: 'rs03',
      bench: {
        position_lower_rad: -1.57,
        position_upper_rad: 1.57,
        torque_limit_nm: 60,
      },
    },
  ],
  control_limits: [],
};

describe('enrichInventory', () => {
  it('returns base unchanged when snapshot is null', () => {
    expect(enrichInventory(robotInventory, null)).toBe(robotInventory);
  });

  it('overlays motor node and limits for matched actuators', () => {
    const enriched = enrichInventory(robotInventory, snapshot);
    const roll = enriched.find((r) => r.name === 'right_shoulder_roll');
    expect(roll?.node).toBe('RS03 · can0 · id 1');
    expect(roll?.limit).toBe('±1.57');
    expect(roll?.preset).toBe('bench_2dof');
  });

  it('does not mutate the base catalog', () => {
    const before = robotInventory.find((r) => r.name === 'right_shoulder_roll')?.node;
    enrichInventory(robotInventory, snapshot);
    expect(robotInventory.find((r) => r.name === 'right_shoulder_roll')?.node).toBe(before);
  });
});
