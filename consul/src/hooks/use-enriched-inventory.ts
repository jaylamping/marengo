import { useMemo } from 'react';
import { useQuery } from '@tanstack/react-query';

import { robotInventory } from '@/data/robot-inventory';
import { enrichInventory } from '@/lib/enrich-inventory';
import { configSnapshotQueryOptions } from '@/hooks/use-config-snapshot';
import { useActuatorStore } from '@/state/actuatorStore';

/**
 * Enriched inventory: disk config for wiring/presets + Davout live hard for Range.
 */
export function useEnrichedInventory() {
  const limitSnapshot = useActuatorStore((s) => s.limitSnapshot);
  const query = useQuery({
    ...configSnapshotQueryOptions,
    placeholderData: null,
  });

  const data = useMemo(
    () => enrichInventory(robotInventory, query.data ?? null, limitSnapshot),
    [query.data, limitSnapshot],
  );

  return { ...query, data };
}
