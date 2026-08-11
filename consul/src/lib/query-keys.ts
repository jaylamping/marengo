/** Shared TanStack Query keys for Consul gateway data. */
export const queryKeys = {
  configSnapshot: ['config', 'snapshot'] as const,
  hardwareCompleteness: ['hardware', 'completeness'] as const,
  commissioningScope: ['hardware', 'commissioning-scope'] as const,
} as const;

/**
 * Keys safe to dehydrate into localStorage.
 *
 * Do **not** persist `config` / `GET /config/snapshot`: motors.yaml hard and
 * control.yaml soft are ADR 0012 write-behind. A throttled persister lets an
 * immediate browser refresh restore pre–Set Limits bounds until a later cycle
 * rewrites the cache — the classic "wiped on refresh, back on next refresh" bug.
 * Live Range SoT is `ActuatorLimitSnapshot` (Zustand, not persisted); disk fields
 * must refetch from the gateway after Durable ACK.
 */
export function isPersistableQueryKey(_queryKey: readonly unknown[]): boolean {
  return false;
}
