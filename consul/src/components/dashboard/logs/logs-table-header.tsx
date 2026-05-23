import { HugeiconsIcon } from '@hugeicons/react';
import { ArrowDown01Icon, ArrowUp01Icon } from '@hugeicons/core-free-icons';

import {
  LOG_TABLE_GRID_CLASS,
  type LogSortField,
  type LogSortState,
} from '@/components/dashboard/logs/constants';
import { cn } from '@/lib/utils';

type LogsTableHeaderProps = {
  sort: LogSortState;
  onSortField: (field: LogSortField) => void;
};

const HEADER_COLUMNS: Array<{ field: LogSortField; label: string }> = [
  { field: 'timestamp', label: 'Time' },
  { field: 'level', label: 'Level' },
  { field: 'source', label: 'Source' },
];

function SortIndicator({
  active,
  direction,
}: {
  active: boolean;
  direction: LogSortState['direction'];
}) {
  if (!active) {
    return null;
  }

  return (
    <HugeiconsIcon
      icon={direction === 'asc' ? ArrowUp01Icon : ArrowDown01Icon}
      strokeWidth={2}
      className="size-3.5 text-muted-foreground"
    />
  );
}

export function LogsTableHeader({ sort, onSortField }: LogsTableHeaderProps) {
  return (
    <div
      className={cn(
        LOG_TABLE_GRID_CLASS,
        'sticky top-0 z-10 border-b bg-muted/40 px-3 py-2 text-xs font-medium backdrop-blur-sm',
      )}
    >
      {HEADER_COLUMNS.map(({ field, label }) => (
        <button
          key={field}
          type="button"
          onClick={() => onSortField(field)}
          className="flex items-center gap-1 text-left hover:text-foreground"
        >
          <span>{label}</span>
          <SortIndicator active={sort.field === field} direction={sort.direction} />
        </button>
      ))}
      <span className="text-muted-foreground">Message</span>
    </div>
  );
}
