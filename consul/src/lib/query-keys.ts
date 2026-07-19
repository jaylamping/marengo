/** Shared TanStack Query keys for Consul gateway data. */
export const queryKeys = {
  configSnapshot: ['config', 'snapshot'] as const,
} as const;

export function isPersistableQueryKey(queryKey: readonly unknown[]): boolean {
  return queryKey[0] === 'config';
}
