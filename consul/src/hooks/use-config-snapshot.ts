import { useQuery } from '@tanstack/react-query';

import { fetchConfigSnapshot, type ConfigSnapshotDto } from '@/lib/config-api';
import { queryKeys } from '@/lib/query-keys';

/** Shared options — enriched inventory selects from this same cache entry. */
export const configSnapshotQueryOptions = {
  queryKey: queryKeys.configSnapshot,
  queryFn: fetchConfigSnapshot,
  /** Always stale: motors/control limits are write-behind and must not linger. */
  staleTime: 0,
} as const;

/** Gateway config snapshot — in-memory Query cache only (not localStorage). */
export function useConfigSnapshot() {
  return useQuery(configSnapshotQueryOptions);
}

export type { ConfigSnapshotDto };
