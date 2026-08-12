export interface Keyframe {
  targetRad: number;
  durationSec: number;
}

export type CompoundAdvanceMode = 'settle' | 'timed';

export type TeachCapability =
  | {
      kind: 'replace-native-wave';
      appliedDescription: string;
      loopFromFirstMotionLandmark: true;
    }
  | {
      kind: 'replace-program';
      appliedDescription: string;
      preserveLoop: boolean;
    };

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
  /** Short UI card copy. */
  description: string;
  /**
   * Kinematic / behavioral brief for Auto Learn prompts.
   * Describes what the gesture should look like in joint language — not UI chrome.
   */
  movementBrief: string;
  joints: string[];
  keyframes: Record<string, Keyframe[]>;
  loop: boolean;
  teach: TeachCapability;
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
 * Flip to `true` only after playbook §4c Wave-pose G-comp PASSes **and** a
 * documented live raise + elbow-wave smoke (`docs/commissioning/limb-playbook.md`).
 * Until then, live (non-dry-run) Wave Start is blocked in the compound panel —
 * Position still carries τ_g, but unsupported elevated Wave raise is not commissioned.
 */
export const WAVE_POSE_GCOMP_SIGNED = false;

/**
 * Wave: raise to playbook `wave_pose`, then continuous elbow_pitch nativeWave
 * (forearm nod). Loop extends nativeWave only (does not re-raise). Taught
 * overlays clear nativeWave and set loopFromSegment.
 *
 * Live Wave raise posts ControlMode.POSITION. That is not "position-only" in the
 * upright-pose sense: Berthier Position includes τ_g feedforward plus impedance
 * (ADR 0007 / docs/safety.md). It does leave GravityComp mode, so Teach Record
 * clears its gravity-armed checkbox. Keep the arm supported until
 * WAVE_POSE_GCOMP_SIGNED. Do not add yaw/elbow to arm_out_forward /
 * arm_fully_up until those playbook gates PASS.
 */
export const COMPOUND_TEST_PRESETS: CompoundTestPreset[] = [
  {
    id: 'wave',
    name: 'Wave',
    description:
      'Arm up to wave_pose (pitch/roll/yaw/elbow under Position+τ_g), then continuous elbow-pitch wave. Taught overlays replace wave phase only after Apply. Live Start blocked until WAVE_POSE_GCOMP_SIGNED.',
    movementBrief:
      'Raise the arm to the commissioned wave_pose: shoulder pitch high, shoulder roll slightly open from the body, upper-arm yaw near zero, elbow bent to a mid raise so the forearm can nod. Then oscillate elbow pitch between the wave extrema while holding pitch, roll, and yaw steady — a recognizable bye-wave (forearm folding), not upper-arm twist or shoulder-roll wag. Teach overlays should encode a raise landmark, then at least two elbow-pitch wave extrema (optional small coordinated roll later). Shipped continuous phase uses native elbow_pitch wave until a teach overlay replaces it.',
    joints: [
      'right_shoulder_pitch',
      'right_shoulder_roll',
      'right_upper_arm_yaw',
      'right_elbow_pitch',
    ],
    loop: true,
    teach: {
      kind: 'replace-native-wave',
      appliedDescription: 'Taught overlay replaces the native wave phase.',
      loopFromFirstMotionLandmark: true,
    },
    advance: 'timed',
    // Matches docs/commissioning/limb-playbook.md `wave_pose` (pitch, roll, yaw, elbow).
    keyframes: {
      right_shoulder_pitch: [{ targetRad: 2.75, durationSec: 3.5 }],
      right_shoulder_roll: [{ targetRad: 0.42, durationSec: 3.5 }],
      right_upper_arm_yaw: [{ targetRad: 0, durationSec: 3.5 }],
      right_elbow_pitch: [{ targetRad: 0.55, durationSec: 3.5 }],
    },
    nativeWave: {
      joint: 'right_elbow_pitch',
      minRad: 0.3,
      maxRad: 0.8,
      // Long enough that Loop does not re-arm mid-swing (re-start was the chop).
      cycles: 50,
      halfPeriodSec: 1.4,
    },
  },
  {
    id: 'arm_out_forward',
    name: 'Arm Out Forward',
    description: 'Raises the arm straight out forward.',
    movementBrief:
      'Reach the arm straight out in front of the body to about shoulder height. Shoulder pitch raises the upper arm forward; shoulder roll opens the arm away from the torso so the hand clears the body. Motion is a single coordinated raise to a held forward pose — no waving or oscillation. Keep both joints moving smoothly toward the forward targets without overshooting stage step budgets.',
    joints: ['right_shoulder_pitch', 'right_shoulder_roll'],
    loop: false,
    teach: {
      kind: 'replace-program',
      appliedDescription:
        'Taught overlay replaces the shipped movement program.',
      preserveLoop: true,
    },
    keyframes: {
      right_shoulder_pitch: [{ targetRad: 1.4, durationSec: 1.5 }],
      right_shoulder_roll: [{ targetRad: 1.57, durationSec: 1.5 }],
    },
  },
  {
    id: 'arm_fully_up',
    name: 'Arm Fully Up',
    description: 'Raises the arm fully up.',
    movementBrief:
      'Raise the arm overhead into a near-vertical reach. Shoulder pitch drives most of the lift from hang toward upright; shoulder roll keeps the arm clear of the torso as it rises. End in a stable elevated pose with both joints held — no wave or side-to-side oscillation. Prefer a progressive raise through one or more intermediate landmarks rather than a single slam from live to peak.',
    joints: ['right_shoulder_pitch', 'right_shoulder_roll'],
    loop: false,
    teach: {
      kind: 'replace-program',
      appliedDescription:
        'Taught overlay replaces the shipped movement program.',
      preserveLoop: true,
    },
    keyframes: {
      right_shoulder_pitch: [{ targetRad: 2.6, durationSec: 2.0 }],
      right_shoulder_roll: [{ targetRad: 1.57, durationSec: 2.0 }],
    },
  },
];

const compoundPresetsById = new Map(
  COMPOUND_TEST_PRESETS.map((preset) => [preset.id, preset]),
);

export function compoundPresetById(id: string): CompoundTestPreset | undefined {
  return compoundPresetsById.get(id);
}
