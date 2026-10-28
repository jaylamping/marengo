import { Keyframe, NativePositionWave } from '@/data/compound-tests';

export function clamp(value: number, min: number, max: number): number {
  return Math.max(min, Math.min(max, value));
}

/** Wall-clock duration of a Berthier PositionWave (cycles × full period). */
export function nativeWaveDurationSec(wave: NativePositionWave): number {
  return wave.cycles * 2 * wave.halfPeriodSec;
}

/**
 * Encode in-loop wave for marengo-pi testing MIT drain.
 * Format: `wave:<joint>:<min>:<max>:<cycles>:<half_period_sec>`
 */
export function encodeNativeWaveCommandName(
  wave: NativePositionWave,
  halfPeriodScale: number = 1
): string {
  const half = Math.max(0.05, wave.halfPeriodSec / Math.max(0.25, halfPeriodScale));
  return `wave:${wave.joint}:${wave.minRad}:${wave.maxRad}:${wave.cycles}:${half}`;
}

export function applyTrimAndClamp(
  position: number,
  trim: number,
  minLimit: number,
  maxLimit: number
): number {
  return clamp(position + trim, minLimit, maxLimit);
}

/** Hold-at settle gate — do not retarget until measured q/vel are near the waypoint.
 *  Bench Wave at pitch≈3.03 / roll target 0.8 plateaus at q≈0.747 (SSE≈0.053) under
 *  gravity FF mismatch — 0.05 never clears; 0.08 matches observed hold error. */
export const WAYPOINT_SETTLE_RAD = 0.08;
export const WAYPOINT_SETTLE_VEL_RAD_S = 0.15;
/** Extra hold after position+velocity settle before allowing retarget. */
export const WAYPOINT_SETTLE_HOLD_SEC = 0.75;

export function jointsSettled(
  measured: Record<string, number | undefined>,
  targets: Record<string, number>,
  settleRad: number = WAYPOINT_SETTLE_RAD,
  measuredVel?: Record<string, number | undefined>,
  settleVelRadS: number = WAYPOINT_SETTLE_VEL_RAD_S
): boolean {
  for (const [name, target] of Object.entries(targets)) {
    const q = measured[name];
    if (q === undefined || Number.isNaN(q)) {
      return false;
    }
    if (Math.abs(q - target) > settleRad) {
      return false;
    }
    if (measuredVel) {
      const v = measuredVel[name];
      if (v === undefined || Number.isNaN(v) || Math.abs(v) > settleVelRadS) {
        return false;
      }
    }
  }
  return true;
}

/** Segment count shared across joints (presets keep parallel keyframe lists). */
export function presetSegmentCount(keyframes: Record<string, Keyframe[]>): number {
  return Math.max(0, ...Object.values(keyframes).map((kfs) => kfs.length));
}

export function segmentDurationSec(
  keyframes: Record<string, Keyframe[]>,
  segmentIndex: number
): number {
  let max = 0;
  for (const kfs of Object.values(keyframes)) {
    const kf = kfs[segmentIndex];
    if (kf && kf.durationSec > max) {
      max = kf.durationSec;
    }
  }
  return max;
}

export function segmentTargets(
  joints: string[],
  keyframes: Record<string, Keyframe[]>,
  segmentIndex: number,
  trims: Record<string, number>,
  limits: Record<string, { min: number; max: number }>
): Record<string, number> {
  const targets: Record<string, number> = {};
  for (const joint of joints) {
    const kf = keyframes[joint]?.[segmentIndex];
    if (!kf) continue;
    const lim = limits[joint] ?? { min: -Math.PI, max: Math.PI };
    targets[joint] = applyTrimAndClamp(kf.targetRad, trims[joint] ?? 0, lim.min, lim.max);
  }
  return targets;
}
