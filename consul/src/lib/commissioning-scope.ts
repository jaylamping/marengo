/**
 * Commissioning scope client helpers (Hardware).
 * Effective scope = persisted ∩ MARENGO_JOINT_SUBSET ceiling (server-side).
 */

export type CommissioningScopeResponse = {
  version: number;
  joints: string[];
  ceiling: string[] | null;
  effective: string[];
  persisted: boolean;
};

/** True when next effective set adds any joint not in previous. */
export function scopeWidens(previousEffective: string[], nextEffective: string[]): boolean {
  const prev = new Set(previousEffective);
  return nextEffective.some((j) => !prev.has(j));
}

export type PutCommissioningScopeBody = {
  joints: string[];
  confirm_widen?: boolean;
};
