import type { BenchProfile } from "./config.js";
import { weightedProfiles } from "./bench-profiles.js";

export const WEIGHTED_PROFILES: BenchProfile[] = weightedProfiles();

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

  // An argument profile can only make the gate stricter (TOOL-04): the
  // launcher's profile describes what is bolted to the bench.
  const weighted =
    requiresWeightedDoubleConfirm(envProfile) ||
    (args.profile !== undefined && requiresWeightedDoubleConfirm(args.profile));
  if (weighted) {
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
