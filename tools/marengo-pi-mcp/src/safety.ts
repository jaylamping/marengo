import type { BenchProfile } from "./config.js";

export const WEIGHTED_PROFILES: BenchProfile[] = [
  "weighted_single_arm",
  "arm_attached",
];

export interface MotionConfirmArgs {
  confirm?: boolean;
  confirm_weighted_motion?: boolean;
  profile?: BenchProfile;
}

export function effectiveProfile(
  envProfile: BenchProfile,
  argProfile?: BenchProfile,
): BenchProfile {
  return argProfile ?? envProfile;
}

export function requiresWeightedDoubleConfirm(profile: BenchProfile): boolean {
  return WEIGHTED_PROFILES.includes(profile);
}

export function validateMotionConfirm(
  args: MotionConfirmArgs,
  envProfile: BenchProfile,
): { ok: true } | { ok: false; message: string } {
  if (args.confirm !== true) {
    return {
      ok: false,
      message:
        "Motion blocked — user must approve; retry with confirm: true",
    };
  }

  const profile = effectiveProfile(envProfile, args.profile);
  if (requiresWeightedDoubleConfirm(profile)) {
    if (args.confirm_weighted_motion !== true) {
      return {
        ok: false,
        message:
          "Weighted motion blocked — requires confirm: true and confirm_weighted_motion: true after two user approvals in chat.",
      };
    }
  }

  return { ok: true };
}
