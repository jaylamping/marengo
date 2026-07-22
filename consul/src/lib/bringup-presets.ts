/** Server registry mirror for client-side membership labels (GET /config/profiles is SoT). */
export const PRESET_TO_PROFILE: Record<string, string> = {
  bench_3dof: 'arm_3dof_right',
  bench_4dof: 'arm_4dof_right',
};

export const PROFILE_TO_PRESET: Record<string, string> = {
  arm_3dof_right: 'bench_3dof',
  arm_4dof_right: 'bench_4dof',
};

export function isMappedBringupPreset(presetId: string): boolean {
  return Object.prototype.hasOwnProperty.call(PRESET_TO_PROFILE, presetId);
}

/** Prefer active profile's mapped bench_* when the joint is a member. */
export function deriveMembershipPreset(
  joint: string,
  activeSlug: string,
  activeJoints: string[],
): string | null {
  if (!activeJoints.includes(joint)) {
    return null;
  }
  return PROFILE_TO_PRESET[activeSlug] ?? null;
}
