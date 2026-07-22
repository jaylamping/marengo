import {
  normalizeOperatorFeedback,
  type AutoLearnJoint,
  type AutoLearnLandmark,
  type AutoLearnLogContext,
  type AutoLearnRequest,
  type AutoLearnStage,
} from '@marengo/compound-auto-learn';
import type { CompoundTestPreset } from '@/data/compound-tests';
import type { ConfigSnapshotDto } from '@/lib/config-api';

export type SnapshotBuildResult =
  | { ok: true; request: AutoLearnRequest }
  | { ok: false; message: string };

type RobotJoint = {
  name: string;
  position: number;
  velocity: number;
  effort: number;
  temperatureC?: number;
};

export function buildAutoLearnRequest(args: {
  preset: CompoundTestPreset;
  stage: AutoLearnStage;
  intent?: string;
  operatorFeedback: string | null;
  config: ConfigSnapshotDto | null;
  robotJoints: RobotJoint[];
  priorLandmarks: AutoLearnLandmark[] | null;
  priorDescription: string | null;
  logContext: AutoLearnLogContext | null;
}): SnapshotBuildResult {
  if (!args.config) {
    return { ok: false, message: 'Config snapshot unavailable' };
  }

  const joints: AutoLearnJoint[] = [];
  const livePositions: Record<string, number> = {};

  for (const name of args.preset.joints) {
    const motor = args.config.motors.find((m) => m.joint === name);
    const control = args.config.control_limits.find((c) => c.joint === name);
    if (!motor?.bench) {
      return {
        ok: false,
        message: `Missing bench limits for ${name}`,
      };
    }
    if (
      control?.velocity_max_rad_s == null ||
      !Number.isFinite(control.velocity_max_rad_s) ||
      control.velocity_max_rad_s <= 0
    ) {
      return {
        ok: false,
        message: `Missing control_limits.velocity_max_rad_s for ${name}`,
      };
    }
    const tele = args.robotJoints.find((j) => j.name === name);
    const positionRad = tele?.position ?? 0;
    livePositions[name] = positionRad;
    joints.push({
      name,
      positionRad,
      velocityRadS: tele?.velocity ?? 0,
      torqueNm: tele?.effort ?? 0,
      tempC: tele?.temperatureC ?? 0,
      positionLowerRad: motor.bench.position_lower_rad,
      positionUpperRad: motor.bench.position_upper_rad,
      velocityMaxRadS: control.velocity_max_rad_s,
      torqueLimitNm: motor.bench.torque_limit_nm,
    });
  }

  return {
    ok: true,
    request: {
      presetId: args.preset.id,
      stage: args.stage,
      intent:
        args.intent ??
        'Propose a safe teach overlay for this preset within stage envelopes',
      operatorFeedback: normalizeOperatorFeedback(args.operatorFeedback),
      joints,
      livePositions,
      priorLandmarks: args.priorLandmarks,
      priorDescription: args.priorDescription,
      logContext: args.logContext,
      base: {
        name: args.preset.name,
        description: args.preset.description,
        movementBrief: args.preset.movementBrief,
        joints: [...args.preset.joints],
        teachKind: args.preset.teach.kind,
        keyframes: args.preset.keyframes,
        nativeWave: args.preset.nativeWave,
      },
    },
  };
}
