import { useQuery } from '@tanstack/react-query';

import { robotInventory } from '@/data/robot-inventory';
import { enrichInventory } from '@/lib/enrich-inventory';
import { configSnapshotQueryOptions } from '@/hooks/use-config-snapshot';

/**
 * Enriched inventory derived from the shared config snapshot query.
 * Persist restores config immediately; select maps it to rows; staleTime 0 refetches.
 */
export function useEnrichedInventory() {
  return useQuery({
    ...configSnapshotQueryOptions,
    // null placeholder → select yields the static catalog before restore/network.
    placeholderData: null,
    select: (snapshot) => enrichInventory(robotInventory, snapshot),
  });
}
