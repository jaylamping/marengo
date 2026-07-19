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
}

export const COMPOUND_TEST_PRESETS: CompoundTestPreset[] = [
  {
    id: 'wave',
    name: 'Wave',
    // Raise to taught pose, then Berthier cosine on roll (no endpoint holds).
    description: 'Arm up, then continuous roll wave.',
    joints: ['right_shoulder_pitch', 'right_shoulder_roll'],
    loop: true,
    advance: 'timed',
    keyframes: {
      right_shoulder_pitch: [{ targetRad: 3.03, durationSec: 3.5 }],
      right_shoulder_roll: [{ targetRad: 0.42, durationSec: 3.5 }],
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
