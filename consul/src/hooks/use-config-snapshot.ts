import { useQuery } from '@tanstack/react-query';

import { fetchConfigSnapshot, type ConfigSnapshotDto } from '@/lib/config-api';
import { queryKeys } from '@/lib/query-keys';

/** Shared options — enriched inventory selects from this same cache entry. */
export const configSnapshotQueryOptions = {
  queryKey: queryKeys.configSnapshot,
  queryFn: fetchConfigSnapshot,
  /** Always treat as stale so mount shows cache then background-refetches. */
  staleTime: 0,
} as const;

/** Gateway config snapshot — cache-first via Query persist, always refetch when stale. */
export function useConfigSnapshot() {
  return useQuery(configSnapshotQueryOptions);
}

export type { ConfigSnapshotDto };
