/**
 * Wired right-arm joints commonly present on the Pi bench (master inventory names).
 * Gateway command allowlist comes from the loaded config dir (`MARENGO_CONFIG_DIR`);
 * operator defaults must not hardcode bringup profile slugs.
 */
export const WIRED_BENCH_JOINTS = [
  'right_shoulder_roll',
  'right_shoulder_pitch',
  'right_upper_arm_yaw',
  'right_elbow_pitch',
] as const;

export type WiredBenchJoint = (typeof WIRED_BENCH_JOINTS)[number];

export type StaticJointLimits = {
  kpMax: number;
  kdMax: number;
  velocityMaxRadS: number;
  tauFfMaxNm: number;
};

/**
 * Display-only reference caps (not a command trust boundary).
 * Commands require a live ActuatorLimitSnapshot from the gateway.
 */
export const DISPLAY_STATIC_JOINT_LIMITS: Record<WiredBenchJoint, StaticJointLimits> = {
  right_shoulder_roll: {
    kpMax: 50,
    kdMax: 5,
    velocityMaxRadS: 2.0,
    tauFfMaxNm: 5.0,
  },
  right_shoulder_pitch: {
    kpMax: 50,
    kdMax: 5,
    velocityMaxRadS: 2.0,
    tauFfMaxNm: 5.0,
  },
  right_upper_arm_yaw: {
    kpMax: 50,
    kdMax: 5,
    velocityMaxRadS: 2.0,
    tauFfMaxNm: 3.0,
  },
  right_elbow_pitch: {
    kpMax: 50,
    kdMax: 5,
    velocityMaxRadS: 1.5,
    tauFfMaxNm: 3.0,
  },
};

export function isWiredBenchJoint(jointName: string): jointName is WiredBenchJoint {
  return (WIRED_BENCH_JOINTS as readonly string[]).includes(jointName);
}

/** Canonical command joint id — same namespace as inventory / robot.yaml. */
export function toCanonicalBenchJoint(jointName: string): string | null {
  if (!isWiredBenchJoint(jointName)) {
    return null;
  }
  return jointName;
}

/** Display-only static caps; never use to arm commands. */
export function staticLimitsForJoint(jointName: string): StaticJointLimits | null {
  if (!isWiredBenchJoint(jointName)) {
    return null;
  }
  return DISPLAY_STATIC_JOINT_LIMITS[jointName];
}

/** Clamp runtime tuning slider values to a known max. */
export function clampTuningValue(value: number, max: number, min = 0): number {
  if (max < min) {
    return min;
  }
  return Math.min(Math.max(value, min), max);
}
