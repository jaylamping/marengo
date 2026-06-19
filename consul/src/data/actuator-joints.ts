/** Four bench-wired joints on the 4-DOF left arm slice (can0 ids 14–17). */
export const WIRED_BENCH_JOINTS = [
  'left_shoulder_roll',
  'left_shoulder_pitch',
  'left_upper_arm_yaw',
  'left_elbow',
] as const;

export type WiredBenchJoint = (typeof WIRED_BENCH_JOINTS)[number];

/** Inventory display name → canonical bench joint id (matches marengo-config alias map). */
export const BENCH_JOINT_ALIASES: Record<WiredBenchJoint, string> = {
  left_shoulder_roll: 'shoulder_roll',
  left_shoulder_pitch: 'shoulder_pitch',
  left_upper_arm_yaw: 'upper_arm_yaw',
  left_elbow: 'elbow',
};

export type StaticJointLimits = {
  kpMax: number;
  kdMax: number;
  velocityMaxRadS: number;
  tauFfMaxNm: number;
};

/** Static Davout caps from config/bringup/arm_4dof_left/control.yaml (fallback until live snapshot). */
export const STATIC_JOINT_LIMITS: Record<WiredBenchJoint, StaticJointLimits> = {
  left_shoulder_roll: {
    kpMax: 5000,
    kdMax: 100,
    velocityMaxRadS: 2.0,
    tauFfMaxNm: 5.0,
  },
  left_shoulder_pitch: {
    kpMax: 5000,
    kdMax: 100,
    velocityMaxRadS: 2.0,
    tauFfMaxNm: 5.0,
  },
  left_upper_arm_yaw: {
    kpMax: 500,
    kdMax: 5,
    velocityMaxRadS: 2.0,
    tauFfMaxNm: 3.0,
  },
  left_elbow: {
    kpMax: 500,
    kdMax: 5,
    velocityMaxRadS: 2.0,
    tauFfMaxNm: 3.0,
  },
};

export function isWiredBenchJoint(jointName: string): jointName is WiredBenchJoint {
  return (WIRED_BENCH_JOINTS as readonly string[]).includes(jointName);
}

export function toCanonicalBenchJoint(jointName: string): string | null {
  if (!isWiredBenchJoint(jointName)) {
    return null;
  }
  return BENCH_JOINT_ALIASES[jointName];
}

export function staticLimitsForJoint(jointName: string): StaticJointLimits | null {
  if (!isWiredBenchJoint(jointName)) {
    return null;
  }
  return STATIC_JOINT_LIMITS[jointName];
}

/** Clamp runtime tuning slider values to live or static caps. */
export function clampTuningValue(value: number, max: number, min = 0): number {
  if (max < min) {
    return min;
  }
  return Math.min(Math.max(value, min), max);
}
