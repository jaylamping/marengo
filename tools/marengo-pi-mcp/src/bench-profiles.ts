/**
 * Single source of truth for MCP bench harness profile metadata.
 * Add a profile here — do not scatter enums across motion/safety/harness.
 *
 * Master config lives at `/opt/marengo/config` (repo `config/`). Limb subsets are
 * ephemeral via `MARENGO_JOINT_SUBSET` — never alternate bringup YAML trees.
 */

export const BENCH_PROFILES = [
  "bare_motor",
  "weighted_single_arm",
  "arm_attached",
  "roll_attached",
  "arm_2dof_smoke",
  "yaw_attached",
  "elbow_attached",
] as const;

export type BenchProfile = (typeof BENCH_PROFILES)[number];

const ROLL = "right_shoulder_roll";
const PITCH = "right_shoulder_pitch";
const YAW = "right_upper_arm_yaw";
const ELBOW = "right_elbow_pitch";

// Pitch-first — matches robot.yaml / URDF chain order.
const RIGHT_ARM_THREE_DOF = [PITCH, ROLL, YAW] as const;
const RIGHT_ARM_FOUR_DOF = [PITCH, ROLL, YAW, ELBOW] as const;
const DUAL_PITCH = ["left_shoulder_pitch", PITCH] as const;

/** Every joint at its mechanical reference (arm/load hanging straight down = 0 rad). */
function hangingAtReference(joints: readonly string[]): Readonly<Record<string, number>> {
  return Object.fromEntries(joints.map((j) => [j, 0]));
}

export interface BenchProfileMeta {
  /** Ephemeral `MARENGO_JOINT_SUBSET` for harness runs on master config. */
  jointSubset?: readonly string[];
  setZeroJoints: string[];
  /** Requires confirm_weighted_motion. */
  weighted: boolean;
  /**
   * Pose (joint → rad) where this profile's load hangs at rest, so the physical gravity
   * torque on every joint is ~0. The gravity gate checks |τ_g| here when no live drive
   * torque is available. Model joints not listed are evaluated at 0.
   */
  hangingRestRad: Readonly<Record<string, number>>;
}

/** Exhaustive map — TypeScript fails if a BenchProfile key is missing. */
export const BENCH_PROFILE_META: Record<BenchProfile, BenchProfileMeta> = {
  bare_motor: {
    setZeroJoints: [...DUAL_PITCH],
    weighted: false,
    hangingRestRad: hangingAtReference(DUAL_PITCH),
  },
  weighted_single_arm: {
    setZeroJoints: [...DUAL_PITCH],
    weighted: true,
    hangingRestRad: hangingAtReference(DUAL_PITCH),
  },
  arm_attached: {
    setZeroJoints: [...DUAL_PITCH],
    weighted: true,
    hangingRestRad: hangingAtReference(DUAL_PITCH),
  },
  roll_attached: {
    jointSubset: RIGHT_ARM_THREE_DOF,
    setZeroJoints: [...RIGHT_ARM_THREE_DOF],
    weighted: true,
    hangingRestRad: hangingAtReference(RIGHT_ARM_THREE_DOF),
  },
  arm_2dof_smoke: {
    jointSubset: RIGHT_ARM_THREE_DOF,
    setZeroJoints: [...RIGHT_ARM_THREE_DOF],
    weighted: true,
    hangingRestRad: hangingAtReference(RIGHT_ARM_THREE_DOF),
  },
  yaw_attached: {
    jointSubset: RIGHT_ARM_FOUR_DOF,
    setZeroJoints: [...RIGHT_ARM_FOUR_DOF],
    weighted: true,
    hangingRestRad: hangingAtReference(RIGHT_ARM_FOUR_DOF),
  },
  elbow_attached: {
    jointSubset: RIGHT_ARM_FOUR_DOF,
    setZeroJoints: [...RIGHT_ARM_FOUR_DOF],
    weighted: true,
    hangingRestRad: hangingAtReference(RIGHT_ARM_FOUR_DOF),
  },
};

export function isBenchProfile(value: string): value is BenchProfile {
  return (BENCH_PROFILES as readonly string[]).includes(value);
}

export function profileMeta(profile: BenchProfile): BenchProfileMeta {
  return BENCH_PROFILE_META[profile];
}

/** Comma-separated `MARENGO_JOINT_SUBSET` for harness SSH sessions. */
export function harnessJointSubset(profile: BenchProfile): string | undefined {
  const subset = profileMeta(profile).jointSubset;
  return subset?.length ? subset.join(",") : undefined;
}

export function weightedProfiles(): BenchProfile[] {
  return BENCH_PROFILES.filter((p) => BENCH_PROFILE_META[p].weighted);
}
