import { describe, expect, it } from 'vitest';

import { create } from '@bufbuild/protobuf';

import {
  ActuatorLimitSnapshotSchema,
  JointActuatorLimitSchema,
} from '@/gen/marengo/v1/marengo_pb';
import { enrichInventory } from '@/lib/enrich-inventory';
import type { ConfigSnapshotDto } from '@/lib/config-api';
import {
  ACTUATOR_NODE_UNAVAILABLE,
  robotInventory,
} from '@/data/robot-inventory';

const snapshot: ConfigSnapshotDto = {
  profile: 'arm_3dof_right',
  config_dir: '/opt/marengo/config',
  joints: ['right_shoulder_roll', 'right_shoulder_pitch', 'right_upper_arm_yaw'],
  motors: [
    {
      joint: 'right_shoulder_roll',
      can_interface: 'can0',
      device_id: 2,
      direction: 1,
      motor_type: 'rs03',
      bench: {
        position_lower_rad: -1.57,
        position_upper_rad: 1.57,
        torque_limit_nm: 60,
      },
    },
  ],
  control_limits: [
    {
      joint: 'right_shoulder_roll',
      position_soft_lower_rad: -1.0,
      position_soft_upper_rad: 1.0,
    },
  ],
};

describe('enrichInventory', () => {
  it('returns base unchanged when snapshot and limits are null', () => {
    expect(enrichInventory(robotInventory, null, null)).toBe(robotInventory);
  });

  it('keeps actuators without hardcoded CAN wiring until a snapshot arrives', () => {
    const roll = robotInventory.find((r) => r.name === 'right_shoulder_roll');
    expect(roll?.node).toBe(ACTUATOR_NODE_UNAVAILABLE);
  });

  it('overlays motor node and disk limits when live snapshot is missing', () => {
    const enriched = enrichInventory(robotInventory, snapshot, null);
    const roll = enriched.find((r) => r.name === 'right_shoulder_roll');
    expect(roll?.node).toBe('RS03 · can0 · id 2');
    expect(roll?.limit).toBe('±1');
    expect(roll?.preset).toBe('bench_3dof');
  });

  it('prefers Davout hard envelope over disk soft for Range', () => {
    const limits = create(ActuatorLimitSnapshotSchema, {
      timestampMs: 1n,
      joints: [
        create(JointActuatorLimitSchema, {
          joint: 'right_shoulder_roll',
          kpMax: 50,
          kdMax: 5,
          velocityMaxRadS: 2,
          tauFfMaxNm: 5,
          posLowerRad: -0.05,
          posUpperRad: 2.58,
          wired: true,
          posSoftLowerRad: 0.0,
          posSoftUpperRad: 2.55,
        }),
      ],
    });
    const enriched = enrichInventory(robotInventory, snapshot, limits);
    const roll = enriched.find((r) => r.name === 'right_shoulder_roll');
    expect(roll?.limit).toBe('-0.05–2.58');
  });

  it('does not mutate the base catalog', () => {
    const before = robotInventory.find((r) => r.name === 'right_shoulder_roll')?.node;
    enrichInventory(robotInventory, snapshot);
    expect(robotInventory.find((r) => r.name === 'right_shoulder_roll')?.node).toBe(
      before,
    );
  });
});
