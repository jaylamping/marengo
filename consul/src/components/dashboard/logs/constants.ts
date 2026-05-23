export const LOG_ROW_HEIGHT_PX = 32;

export const LOG_TABLE_GRID_CLASS =
  'grid grid-cols-[minmax(180px,0.35fr)_88px_minmax(120px,0.2fr)_minmax(0,1fr)]';

export const LOG_LEVEL_STYLES = {
  DEBUG: 'text-muted-foreground',
  INFO: 'text-foreground',
  WARN: 'text-amber-400',
  ERROR: 'text-red-400',
  FATAL: 'text-red-500',
} as const;

export const LOG_LEVEL_BADGE_VARIANT = {
  DEBUG: 'outline',
  INFO: 'secondary',
  WARN: 'outline',
  ERROR: 'destructive',
  FATAL: 'destructive',
} as const;

export type LogSortField = 'timestamp' | 'level' | 'source';
export type LogSortDirection = 'asc' | 'desc';

export type LogSortState = {
  field: LogSortField;
  direction: LogSortDirection;
};

export const DEFAULT_LOG_SORT: LogSortState = {
  field: 'timestamp',
  direction: 'desc',
};

const LEVEL_RANK = {
  DEBUG: 0,
  INFO: 1,
  WARN: 2,
  ERROR: 3,
  FATAL: 4,
} as const;

export function compareLogEntries(
  left: { timestamp: number; level: keyof typeof LEVEL_RANK; source: string },
  right: { timestamp: number; level: keyof typeof LEVEL_RANK; source: string },
  sort: LogSortState,
): number {
  let result = 0;

  switch (sort.field) {
    case 'timestamp':
      result = left.timestamp - right.timestamp;
      break;
    case 'level':
      result = LEVEL_RANK[left.level] - LEVEL_RANK[right.level];
      break;
    case 'source':
      result = left.source.localeCompare(right.source);
      break;
  }

  return sort.direction === 'asc' ? result : -result;
}

export function toggleLogSort(
  current: LogSortState,
  field: LogSortField,
): LogSortState {
  if (current.field === field) {
    return {
      field,
      direction: current.direction === 'asc' ? 'desc' : 'asc',
    };
  }

  return {
    field,
    direction: field === 'timestamp' ? 'desc' : 'asc',
  };
}
