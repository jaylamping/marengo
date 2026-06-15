import { useMemo, useSyncExternalStore } from 'react';

import { useLogsFilter } from '@/components/dashboard/logs/logs-filter-context';
import {
  buildLevelOnlyVisibleIndices,
  directVisibleLogicalIndex,
  getVisibleLogIndices,
  usesDirectVisibleIndex,
  usesLevelOnlyVisibleIndex,
} from '@/lib/log-view-index';
import { logBuffer } from '@/lib/log-buffer';
import { probeVisibleIndexRebuild } from '@/lib/log-debug-probe';

export type VisibleLogIndexModel =
  | {
      mode: 'direct';
      count: number;
      version: number;
      getLogicalIndex: (displayIndex: number) => number;
    }
  | {
      mode: 'filtered';
      indices: readonly number[];
      version: number;
    };

export function useVisibleLogIndexModel(): VisibleLogIndexModel {
  const snapshot = useSyncExternalStore(
    logBuffer.subscribe,
    logBuffer.getSnapshot,
    logBuffer.getSnapshot,
  );
  const { levelFilter, deferredSearchQuery, sort } = useLogsFilter();

  const direct = usesDirectVisibleIndex(
    levelFilter,
    deferredSearchQuery,
    sort,
  );

  const filteredIndices = useMemo(
    () => {
      if (direct) {
        return [];
      }
      probeVisibleIndexRebuild();
      const indices = usesLevelOnlyVisibleIndex(
        levelFilter,
        deferredSearchQuery,
        sort,
      )
        ? buildLevelOnlyVisibleIndices(
            snapshot.count,
            (index) => logBuffer.getEntry(index),
            levelFilter,
            sort,
          )
        : getVisibleLogIndices(
            snapshot.version,
            snapshot.count,
            (index) => logBuffer.getEntry(index),
            levelFilter,
            deferredSearchQuery,
            sort,
          );
      return indices;
    },
    [
      deferredSearchQuery,
      direct,
      levelFilter,
      snapshot.count,
      snapshot.version,
      sort,
    ],
  );

  if (direct) {
    return {
      mode: 'direct',
      count: snapshot.count,
      version: snapshot.version,
      getLogicalIndex: (displayIndex: number) =>
        directVisibleLogicalIndex(displayIndex, snapshot.count, sort),
    };
  }

  return {
    mode: 'filtered',
    indices: filteredIndices,
    version: snapshot.version,
  };
}

/** @deprecated Prefer useVisibleLogIndexModel */
export function useVisibleLogIndices(): readonly number[] {
  const model = useVisibleLogIndexModel();
  if (model.mode === 'filtered') {
    return model.indices;
  }
  const indices = new Array<number>(model.count);
  for (let index = 0; index < model.count; index += 1) {
    indices[index] = model.getLogicalIndex(index);
  }
  return indices;
}
