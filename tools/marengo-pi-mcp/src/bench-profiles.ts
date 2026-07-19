/**
 * Single source of truth for MCP bench harness profile metadata.
 * Add a profile here — do not scatter enums across motion/safety/harness.
 */

export const BENCH_PROFILES = [
  "bare_motor",
  "weighted_single_arm",
  "arm_attached",
  "roll_attached",
  "arm_2dof_smoke",
  "yaw_attached",
] as const;

export type BenchProfile = (typeof BENCH_PROFILES)[number];

const ROLL = "right_shoulder_roll";
const PITCH = "right_shoulder_pitch";
const YAW = "right_upper_arm_yaw";

export type BenchBringupSlug = "arm_3dof_right" | "shoulder_pitch_weighted";

export interface BenchProfileMeta {
  /** Relative bringup slug under config/bringup/; omit = use MARENGO_CONFIG_DIR. */
  configBringup?: BenchBringupSlug;
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
    configBringup: "shoulder_pitch_weighted",
    setZeroJoints: ["left_shoulder_pitch", "right_shoulder_pitch"],
    weighted: true,
    skipGravityPreview: false,
  },
  arm_attached: {
    configBringup: "shoulder_pitch_weighted",
    setZeroJoints: ["left_shoulder_pitch", "right_shoulder_pitch"],
    weighted: true,
    skipGravityPreview: false,
  },
  roll_attached: {
    configBringup: "arm_3dof_right",
    setZeroJoints: [ROLL, PITCH, YAW],
    weighted: true,
    skipGravityPreview: true,
  },
  arm_2dof_smoke: {
    configBringup: "arm_3dof_right",
    setZeroJoints: [ROLL, PITCH, YAW],
    weighted: true,
    skipGravityPreview: true,
  },
  yaw_attached: {
    configBringup: "arm_3dof_right",
    setZeroJoints: [ROLL, PITCH, YAW],
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
  return profileMeta(profile).configBringup === "arm_3dof_right";
}

export function weightedProfiles(): BenchProfile[] {
  return BENCH_PROFILES.filter((p) => BENCH_PROFILE_META[p].weighted);
}

