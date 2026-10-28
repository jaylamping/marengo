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
  ENABLE_ACTIVE_HOLD_MS,
  ENABLE_OUTCOME_TIMEOUT_MS,
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

  it('keeps waiting while DISABLED before ACTIVE was ever seen (slow Enable)', () => {
    // Preflight, IPC, the Enable stagger and a post-SetZero quiet can all
    // keep the mode DISABLED past 700 ms on a healthy Enable.
    for (const elapsedMs of [700, 900, 2500, ENABLE_OUTCOME_TIMEOUT_MS - 1]) {
      const result = interpretPostEnableWatch({
        elapsedMs,
        operationalMode: 'DISABLED',
        safetyState: null,
        activeForMs: null,
      });
      expect(result.done).toBe(false);
      expect(result.kind).toBe('pending');
    }
  });

  it('flags a drop to DISABLED after ACTIVE was seen', () => {
    const result = interpretPostEnableWatch({
      elapsedMs: 900,
      operationalMode: 'DISABLED',
      safetyState: null,
      activeForMs: 100,
    });
    expect(result.done).toBe(true);
    expect(result.kind).toBe('error');
    expect(result.message).toMatch(/returned to DISABLED/i);
  });

  it('flags an Enable that never reaches ACTIVE once the Pi bound has passed', () => {
    expect(ENABLE_OUTCOME_TIMEOUT_MS).toBeGreaterThanOrEqual(2000 + 200);
    const result = interpretPostEnableWatch({
      elapsedMs: ENABLE_OUTCOME_TIMEOUT_MS,
      operationalMode: 'DISABLED',
      safetyState: null,
      activeForMs: null,
    });
    expect(result.done).toBe(true);
    expect(result.kind).toBe('error');
    expect(result.message).toMatch(/did not reach ACTIVE/i);
  });

  it('reports a published enable refusal at once', () => {
    const result = interpretPostEnableWatch({
      elapsedMs: 100,
      operationalMode: 'DISABLED',
      safetyState: null,
      enableRefusal: 'enable refused: motion is owned by stdin, not chappe',
    });
    expect(result.done).toBe(true);
    expect(result.kind).toBe('error');
    expect(result.message).toMatch(/owned by stdin/);
  });

  it('confirms ACTIVE after a short hold', () => {
    const result = interpretPostEnableWatch({
      elapsedMs: 1500,
      operationalMode: 'ACTIVE',
      safetyState: null,
      activeForMs: ENABLE_ACTIVE_HOLD_MS,
    });
    expect(result.done).toBe(true);
    expect(result.kind).toBe('ok');
  });

  it('a late ACTIVE is held before it is confirmed', () => {
    const result = interpretPostEnableWatch({
      elapsedMs: 3000,
      operationalMode: 'ACTIVE',
      safetyState: null,
      activeForMs: 200,
    });
    expect(result.done).toBe(false);
    expect(result.kind).toBe('pending');
  });
});

describe('safety publication honesty (L-armee-proto-07)', () => {
  it('names E-stop severity instead of rendering it as a plain fault', () => {
    const estop = create(FaultSchema, {
      code: 'runtime',
      message: 'hardware estop',
      severity: FaultSeverity.ESTOP,
    });
    expect(formatSafetyFaults([estop])).toMatch(/^E-STOP /);
  });

  it('reports an asserted hardware E-stop even with an empty fault list', () => {
    const safetyState = create(SafetyStateSchema, {
      mode: OperationalMode.DISABLED,
      hardwareEstopAsserted: true,
    });
    const verdict = interpretPostEnableWatch({
      elapsedMs: 100,
      operationalMode: 'DISABLED',
      safetyState,
    });
    expect(verdict.kind).toBe('error');
    expect(verdict.message).toContain('E-stop');
  });
});
