/** Per-joint caps as published by Davout in the live ActuatorLimitSnapshot. */
export type JointCapLimits = {
  kpMax: number;
  kdMax: number;
  velocityMaxRadS: number;
  tauFfMaxNm: number;
};

/** Clamp runtime tuning slider values to a known max. */
export function clampTuningValue(value: number, max: number, min = 0): number {
  if (max < min) {
    return min;
  }
  return Math.min(Math.max(value, min), max);
}
