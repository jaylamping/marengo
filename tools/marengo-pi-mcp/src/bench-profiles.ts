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

const RIGHT_ARM_THREE_DOF = [ROLL, PITCH, YAW] as const;
const RIGHT_ARM_FOUR_DOF = [ROLL, PITCH, YAW, ELBOW] as const;

export interface BenchProfileMeta {
  /** Ephemeral `MARENGO_JOINT_SUBSET` for harness runs on master config. */
  jointSubset?: readonly string[];
  setZeroJoints: string[];
  /** Requires confirm_weighted_motion. */
  weighted: boolean;
  /** Skip motor-repl gravity-preview in harness preflight. */
  skipGravityPreview: boolean;
}

/** Exhaustive map — TypeScript fails if a BenchProfile key is missing. */
export const BENCH_PROFILE_META: Record<BenchProfile, BenchProfileMeta> = {
  bare_motor: {
    setZeroJoints: ["left_shoulder_pitch", "right_shoulder_pitch"],
    weighted: false,
    skipGravityPreview: false,
  },
  weighted_single_arm: {
    setZeroJoints: ["left_shoulder_pitch", "right_shoulder_pitch"],
    weighted: true,
    skipGravityPreview: false,
  },
  arm_attached: {
    setZeroJoints: ["left_shoulder_pitch", "right_shoulder_pitch"],
    weighted: true,
    skipGravityPreview: false,
  },
  roll_attached: {
    jointSubset: RIGHT_ARM_THREE_DOF,
    setZeroJoints: [...RIGHT_ARM_THREE_DOF],
    weighted: true,
    skipGravityPreview: true,
  },
  arm_2dof_smoke: {
    jointSubset: RIGHT_ARM_THREE_DOF,
    setZeroJoints: [...RIGHT_ARM_THREE_DOF],
    weighted: true,
    skipGravityPreview: true,
  },
  yaw_attached: {
    jointSubset: RIGHT_ARM_FOUR_DOF,
    setZeroJoints: [...RIGHT_ARM_FOUR_DOF],
    weighted: true,
    skipGravityPreview: true,
  },
  elbow_attached: {
    jointSubset: RIGHT_ARM_FOUR_DOF,
    setZeroJoints: [...RIGHT_ARM_FOUR_DOF],
    weighted: true,
    skipGravityPreview: true,
  },
};

export function isBenchProfile(value: string): value is BenchProfile {
  return (BENCH_PROFILES as readonly string[]).includes(value);
}

export function profileMeta(profile: BenchProfile): BenchProfileMeta {
  return BENCH_PROFILE_META[profile];
}

export function isRightArmBenchProfile(profile: BenchProfile): boolean {
  return profileMeta(profile).setZeroJoints.some((j) => j.startsWith("right_"));
}

/** Comma-separated `MARENGO_JOINT_SUBSET` for harness SSH sessions. */
export function harnessJointSubset(profile: BenchProfile): string | undefined {
  const subset = profileMeta(profile).jointSubset;
  return subset?.length ? subset.join(",") : undefined;
}

export function weightedProfiles(): BenchProfile[] {
  return BENCH_PROFILES.filter((p) => BENCH_PROFILE_META[p].weighted);
}
