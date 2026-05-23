import { useMemo, useSyncExternalStore } from 'react';

import { useLogsFilter } from '@/components/dashboard/logs/logs-filter-context';
import { getVisibleLogIndices } from '@/lib/log-view-index';
import { logBuffer } from '@/lib/log-buffer';

export function useVisibleLogIndices(): readonly number[] {
  const snapshot = useSyncExternalStore(
    logBuffer.subscribe,
    logBuffer.getSnapshot,
    logBuffer.getSnapshot,
  );
  const { levelFilter, deferredSearchQuery, sort } = useLogsFilter();

  return useMemo(
    () =>
      getVisibleLogIndices(
        snapshot.version,
        snapshot.count,
        (index) => logBuffer.getEntry(index),
        levelFilter,
        deferredSearchQuery,
        sort,
      ),
    [
      deferredSearchQuery,
      levelFilter,
      snapshot.count,
      snapshot.version,
      sort,
    ],
  );
}
