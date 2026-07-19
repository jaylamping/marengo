export interface Keyframe {
  targetRad: number;
  durationSec: number;
}

export type CompoundAdvanceMode = 'settle' | 'timed';

/** In-loop Berthier cosine wave (via testing MIT name `wave:…`). */
export interface NativePositionWave {
  joint: string;
  minRad: number;
  maxRad: number;
  cycles: number;
  halfPeriodSec: number;
}

export interface CompoundTestPreset {
  id: string;
  name: string;
  description: string;
  joints: string[];
  keyframes: Record<string, Keyframe[]>;
  loop: boolean;
  /**
   * How keyframe segments advance.
   * - settle (default): dwell + measured near target.
   * - timed: dwell only.
   * When `nativeWave` is set, keyframes are the raise only; waving is continuous in Berthier.
   */
  advance?: CompoundAdvanceMode;
  /** After raise keyframes finish, start Berthier PositionWave on this joint. */
  nativeWave?: NativePositionWave;
  /**
   * When Loop is on and keyframes finish (no nativeWave), restart at this segment
   * instead of 0. Used by taught overlays so raise is not repeated.
   */
  loopFromSegment?: number;
}

/**
 * Shipped Wave: raise includes yaw + elbow pitch; roll wave stays nativeWave
 * until a teach overlay replaces the wave phase. Loop extends nativeWave only
 * (does not re-raise). Taught overlays clear nativeWave and set loopFromSegment.
 *
 * Live Wave raise uses Position (ends GravityComp) — keep the arm supported
 * until Wave-pose G-comp sign is commissioned (docs/bench-elbow-test-suite.md E6).
 * Do not add yaw/elbow to arm_out_forward / arm_fully_up until Y3–Y4 / E gates PASS.
 */
export const COMPOUND_TEST_PRESETS: CompoundTestPreset[] = [
  {
    id: 'wave',
    name: 'Wave',
    description:
      'Arm up (pitch/roll/yaw/elbow raise), then continuous roll wave. Taught overlays replace wave phase only after Apply. Support the arm until E6 Wave-pose G-comp is signed.',
    joints: [
      'right_shoulder_pitch',
      'right_shoulder_roll',
      'right_upper_arm_yaw',
      'right_elbow_pitch',
    ],
    loop: true,
    advance: 'timed',
    keyframes: {
      right_shoulder_pitch: [{ targetRad: 3.03, durationSec: 3.5 }],
      right_shoulder_roll: [{ targetRad: 0.42, durationSec: 3.5 }],
      right_upper_arm_yaw: [{ targetRad: 0, durationSec: 3.5 }],
      right_elbow_pitch: [{ targetRad: 1.0, durationSec: 3.5 }],
    },
    nativeWave: {
      joint: 'right_shoulder_roll',
      minRad: 0.42,
      maxRad: 0.7,
      // Long enough that Loop does not re-arm mid-swing (re-start was the chop).
      // halfPeriod 1.4s: peak |dq| ≈ 0.31 rad/s — a bit faster than 1.6s, still under choppy 1.2s (~0.37).
      cycles: 50,
      halfPeriodSec: 1.4,
    },
  },
  {
    id: 'arm_out_forward',
    name: 'Arm Out Forward',
    description: 'Raises the arm straight out forward.',
    joints: ['right_shoulder_pitch', 'right_shoulder_roll'],
    loop: false,
    keyframes: {
      right_shoulder_pitch: [{ targetRad: 1.4, durationSec: 1.5 }],
      right_shoulder_roll: [{ targetRad: 1.57, durationSec: 1.5 }],
    },
  },
  {
    id: 'arm_fully_up',
    name: 'Arm Fully Up',
    description: 'Raises the arm fully up.',
    joints: ['right_shoulder_pitch', 'right_shoulder_roll'],
    loop: false,
    keyframes: {
      right_shoulder_pitch: [{ targetRad: 2.6, durationSec: 2.0 }],
      right_shoulder_roll: [{ targetRad: 1.57, durationSec: 2.0 }],
    },
  },
];
