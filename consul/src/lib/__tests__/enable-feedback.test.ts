import { describe, expect, it } from 'vitest';

import { create } from '@bufbuild/protobuf';
import {
  ActuatorLimitSnapshotSchema,
  FaultSchema,
  FaultSeverity,
  JointActuatorLimitSchema,
  JointStateSchema,
  OperationalMode,
  RobotStateSchema,
  SafetyStateSchema,
} from '@/gen/marengo/v1/marengo_pb';
import type { ConfigSnapshotDto } from '@/lib/config-api';
import {
  diagnoseEnableDisabledTrip,
  formatSafetyFaults,
  interpretPostEnableWatch,
} from '@/lib/enable-feedback';

describe('formatSafetyFaults', () => {
  it('joins joint + message', () => {
    const fault = create(FaultSchema, {
      code: 'hard_limit',
      message: 'outside [-0.05, 1.2]',
      severity: FaultSeverity.FAULT,
      joint: 'right_elbow_pitch',
    });
    expect(formatSafetyFaults([fault])).toContain('right_elbow_pitch');
    expect(formatSafetyFaults([fault])).toContain('outside');
  });
});

describe('diagnoseEnableDisabledTrip', () => {
  const config: ConfigSnapshotDto = {
    profile: 'arm_4dof_right',
    config_dir: '/opt/marengo/config',
    joints: ['right_elbow_pitch'],
    motors: [
      {
        joint: 'right_elbow_pitch',
        can_interface: 'can0',
        device_id: 4,
        direction: 1,
        motor_type: 'RS02',
        bench: {
          position_lower_rad: -0.5,
          position_upper_rad: 0.95,
          torque_limit_nm: 3,
        },
      },
    ],
    control_limits: [],
  };

  const liveHard = create(ActuatorLimitSnapshotSchema, {
    timestampMs: 1n,
    joints: [
      create(JointActuatorLimitSchema, {
        joint: 'right_elbow_pitch',
        kpMax: 50,
        kdMax: 5,
        velocityMaxRadS: 1.5,
        tauFfMaxNm: 3,
        posLowerRad: -0.4,
        posUpperRad: 1.0,
        wired: true,
        posSoftLowerRad: -0.37,
        posSoftUpperRad: 0.97,
      }),
    ],
  });

  it('names joints outside Davout hard when live snapshot is present', () => {
    const robot = create(RobotStateSchema, {
      timestampMs: 0n,
      joints: [
        create(JointStateSchema, {
          name: 'right_elbow_pitch',
          position: -0.45,
        }),
      ],
    });
    // Inside motors.yaml bench (-0.5) but outside live Davout hard (-0.4).
    expect(diagnoseEnableDisabledTrip(robot, config, liveHard)).toMatch(
      /outside Davout hard/i,
    );
  });

  it('falls back to motors bench when live snapshot is missing', () => {
    const robot = create(RobotStateSchema, {
      timestampMs: 0n,
      joints: [
        create(JointStateSchema, {
          name: 'right_elbow_pitch',
          position: -0.6,
        }),
      ],
    });
    expect(diagnoseEnableDisabledTrip(robot, config)).toMatch(
      /outside motors bench/i,
    );
  });

  it('points at journal when poses sit inside hard envelope', () => {
    const robot = create(RobotStateSchema, {
      timestampMs: 0n,
      joints: [
        create(JointStateSchema, {
          name: 'right_elbow_pitch',
          position: -0.23,
        }),
      ],
    });
    expect(diagnoseEnableDisabledTrip(robot, config, liveHard)).toMatch(
      /Pi journal/i,
    );
  });
});

describe('interpretPostEnableWatch', () => {
  it('reports safety faults immediately', () => {
    const safety = create(SafetyStateSchema, {
      timestampMs: 0n,
      mode: OperationalMode.DISABLED,
      activeFaults: [
        create(FaultSchema, {
          code: 'hard_limit',
          message: 'position outside limits',
          severity: FaultSeverity.FAULT,
          joint: 'right_elbow_pitch',
        }),
      ],
    });
    const result = interpretPostEnableWatch({
      elapsedMs: 100,
      operationalMode: 'DISABLED',
      safetyState: safety,
    });
    expect(result.done).toBe(true);
    expect(result.kind).toBe('error');
    expect(result.message).toMatch(/tripped safety/i);
  });

  it('flags DISABLED after enable without faults', () => {
    const result = interpretPostEnableWatch({
      elapsedMs: 900,
      operationalMode: 'DISABLED',
      safetyState: null,
    });
    expect(result.done).toBe(true);
    expect(result.kind).toBe('error');
    expect(result.message).toMatch(/DISABLED/i);
  });

  it('confirms ACTIVE after a short hold', () => {
    const result = interpretPostEnableWatch({
      elapsedMs: 1500,
      operationalMode: 'ACTIVE',
      safetyState: null,
    });
    expect(result.done).toBe(true);
    expect(result.kind).toBe('ok');
  });
});
