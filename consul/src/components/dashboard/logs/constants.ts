import { dashboardPanelPointerClassName } from '@/components/dashboard/layout/constants';

const logsPanelSurfaceClassName = [
  'border border-line bg-surface-1',
  'shadow-[0_8px_24px_rgb(0_0_0/0.3)]',
].join(' ');

export const logsToolbarShellClassName = [
  'flex flex-col gap-4 rounded-lg px-4 py-3 lg:px-6',
  dashboardPanelPointerClassName,
  logsPanelSurfaceClassName,
].join(' ');

export const logsTableShellClassName = [
  'flex min-h-0 flex-1 flex-col overflow-hidden rounded-lg',
  dashboardPanelPointerClassName,
  logsPanelSurfaceClassName,
].join(' ');

export const logsSessionListShellClassName = [
  'flex max-h-[480px] flex-col gap-1 overflow-auto rounded-lg p-2',
  dashboardPanelPointerClassName,
  logsPanelSurfaceClassName,
].join(' ');

export const logsArchivePanelShellClassName = [
  'flex min-h-0 flex-1 flex-col overflow-hidden rounded-lg p-3',
  dashboardPanelPointerClassName,
  logsPanelSurfaceClassName,
].join(' ');

export const logsSheetContentClassName = [
  'border border-line bg-surface-1',
  'shadow-[0_8px_32px_rgb(0_0_0/0.5)]',
].join(' ');

export const logsTabsVariant = 'panel' as const;

export const LOG_ROW_HEIGHT_PX = 32;

export const LOG_TABLE_GRID_CLASS =
  'grid grid-cols-[minmax(180px,0.35fr)_88px_minmax(120px,0.2fr)_minmax(0,1fr)]';

export const LOG_LEVEL_STYLES = {
  DEBUG: 'text-muted-foreground',
  INFO: 'text-foreground',
  WARN: 'text-warning',
  ERROR: 'text-fault',
  FATAL: 'text-fault',
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
