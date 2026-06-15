import type { LogEntry, LogLevelFilter } from '@/data/logs';

import {
  compareLogEntries,
  type LogSortState,
} from '@/components/dashboard/logs/constants';

type GetEntry = (index: number) => LogEntry | undefined;

function matchesSearch(entry: LogEntry, query: string): boolean {
  const normalized = query.trim().toLowerCase();
  if (normalized.length === 0) {
    return true;
  }

  return (
    entry.message.toLowerCase().includes(normalized) ||
    entry.source.toLowerCase().includes(normalized) ||
    entry.level.toLowerCase().includes(normalized)
  );
}

export function buildFilterKey(
  levelFilter: LogLevelFilter,
  searchQuery: string,
  sort: LogSortState,
): string {
  return `${levelFilter}|${searchQuery.trim().toLowerCase()}|${sort.field}|${sort.direction}`;
}

export function usesDirectVisibleIndex(
  levelFilter: LogLevelFilter,
  searchQuery: string,
  sort: LogSortState,
): boolean {
  if (levelFilter !== 'all' || searchQuery.trim().length > 0) {
    return false;
  }
  return sort.field === 'timestamp';
}

export function directVisibleLogicalIndex(
  displayIndex: number,
  count: number,
  sort: LogSortState,
): number {
  if (sort.direction === 'desc') {
    return count - 1 - displayIndex;
  }
  return displayIndex;
}

export function buildVisibleLogIndices(
  count: number,
  getEntry: GetEntry,
  levelFilter: LogLevelFilter,
  searchQuery: string,
  sort: LogSortState,
): number[] {
  if (count === 0) {
    return [];
  }

  const normalizedSearch = searchQuery.trim().toLowerCase();
  const hasSearch = normalizedSearch.length > 0;
  const hasLevelFilter = levelFilter !== 'all';

  if (!hasSearch && !hasLevelFilter) {
    if (sort.field === 'timestamp' && sort.direction === 'desc') {
      const indices = new Array<number>(count);
      for (let index = 0; index < count; index += 1) {
        indices[index] = count - 1 - index;
      }
      return indices;
    }

    if (sort.field === 'timestamp' && sort.direction === 'asc') {
      const indices = new Array<number>(count);
      for (let index = 0; index < count; index += 1) {
        indices[index] = index;
      }
      return indices;
    }
  }

  const indices: number[] = [];

  for (let index = 0; index < count; index += 1) {
    const entry = getEntry(index);
    if (!entry) {
      continue;
    }

    if (hasLevelFilter && entry.level !== levelFilter) {
      continue;
    }

    if (hasSearch && !matchesSearch(entry, normalizedSearch)) {
      continue;
    }

    indices.push(index);
  }

  if (indices.length <= 1) {
    return indices;
  }

  indices.sort((leftIndex, rightIndex) => {
    const left = getEntry(leftIndex);
    const right = getEntry(rightIndex);
    if (!left || !right) {
      return 0;
    }

    return compareLogEntries(left, right, sort);
  });

  return indices;
}

type VisibleIndexCache = {
  version: number;
  count: number;
  filterKey: string;
  indices: number[];
};

let visibleIndexCache: VisibleIndexCache | null = null;

export function getVisibleLogIndices(
  version: number,
  count: number,
  getEntry: GetEntry,
  levelFilter: LogLevelFilter,
  searchQuery: string,
  sort: LogSortState,
): number[] {
  const filterKey = buildFilterKey(levelFilter, searchQuery, sort);

  if (
    visibleIndexCache &&
    visibleIndexCache.version === version &&
    visibleIndexCache.count === count &&
    visibleIndexCache.filterKey === filterKey
  ) {
    return visibleIndexCache.indices;
  }

  const indices = buildVisibleLogIndices(
    count,
    getEntry,
    levelFilter,
    searchQuery,
    sort,
  );

  visibleIndexCache = {
    version,
    count,
    filterKey,
    indices,
  };

  return indices;
}

export function invalidateVisibleLogIndexCache() {
  visibleIndexCache = null;
}
