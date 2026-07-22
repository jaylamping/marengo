// @vitest-environment node
import { describe, expect, it } from 'vitest';

import { COMPOUND_TEST_PRESETS } from '@/data/compound-tests';
import { buildAutoLearnRequest } from '@/lib/auto-learn-snapshot';
import type { ConfigSnapshotDto } from '@/lib/config-api';

const armOut = COMPOUND_TEST_PRESETS.find((p) => p.id === 'arm_out_forward');

function configWithLimits(
  opts: {
    omitBench?: string;
    omitVelocity?: string;
  } = {},
): ConfigSnapshotDto {
  const joints = armOut!.joints;
  return {
    profile: 'arm_4dof_right',
    config_dir: '/opt/marengo/config',
    joints: [...joints],
    motors: joints.map((joint) => {
      const entry = {
        joint,
        can_interface: 'can0',
        device_id: 1,
        direction: 1,
        motor_type: 'rs00',
        bench: {
          position_lower_rad: -1,
          position_upper_rad: 3.2,
          torque_limit_nm: 15,
        },
      };
      if (opts.omitBench === joint) {
        const { bench: _bench, ...rest } = entry;
        return rest as ConfigSnapshotDto['motors'][number];
      }
      return entry;
    }),
    control_limits: joints.map((joint) => ({
      joint,
      velocity_max_rad_s: opts.omitVelocity === joint ? undefined : 2,
    })),
  };
}

describe('buildAutoLearnRequest', () => {
  it('fails closed without config', () => {
    const result = buildAutoLearnRequest({
      preset: armOut!,
      stage: 'crawl',
      operatorFeedback: null,
      config: null,
      robotJoints: [],
      priorLandmarks: null,
      priorDescription: null,
      logContext: null,
    });
    expect(result.ok).toBe(false);
  });

  it('fails closed without bench limits', () => {
    const joint = armOut!.joints[0]!;
    const result = buildAutoLearnRequest({
      preset: armOut!,
      stage: 'crawl',
      operatorFeedback: null,
      config: configWithLimits({ omitBench: joint }),
      robotJoints: [],
      priorLandmarks: null,
      priorDescription: null,
      logContext: null,
    });
    expect(result.ok).toBe(false);
    if (!result.ok) {
      expect(result.message).toContain('bench');
    }
  });

  it('fails closed without control_limits velocity', () => {
    const joint = armOut!.joints[0]!;
    const result = buildAutoLearnRequest({
      preset: armOut!,
      stage: 'crawl',
      operatorFeedback: null,
      config: configWithLimits({ omitVelocity: joint }),
      robotJoints: [],
      priorLandmarks: null,
      priorDescription: null,
      logContext: null,
    });
    expect(result.ok).toBe(false);
    if (!result.ok) {
      expect(result.message).toContain('velocity_max_rad_s');
    }
  });

  it('builds request with live positions and effort→torque', () => {
    const result = buildAutoLearnRequest({
      preset: armOut!,
      stage: 'crawl',
      operatorFeedback: '  slower on pitch  ',
      config: configWithLimits(),
      robotJoints: armOut!.joints.map((name) => ({
        name,
        position: 0.25,
        velocity: 0.01,
        effort: 1.5,
        temperatureC: 30,
      })),
      priorLandmarks: null,
      priorDescription: null,
      logContext: null,
    });
    expect(result.ok).toBe(true);
    if (result.ok) {
      expect(result.request.operatorFeedback).toBe('slower on pitch');
      expect(result.request.joints[0]?.torqueNm).toBe(1.5);
      expect(result.request.livePositions[armOut!.joints[0]!]).toBe(0.25);
    }
  });
});
