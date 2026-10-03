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

/** Per-joint caps as published by Davout in the live ActuatorLimitSnapshot. */
export type JointCapLimits = {
  kpMax: number;
  kdMax: number;
  velocityMaxRadS: number;
  tauFfMaxNm: number;
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

/** Clamp runtime tuning slider values to a known max. */
export function clampTuningValue(value: number, max: number, min = 0): number {
  if (max < min) {
    return min;
  }
  return Math.min(Math.max(value, min), max);
}
