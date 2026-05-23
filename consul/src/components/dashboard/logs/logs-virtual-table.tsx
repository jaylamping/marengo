import { memo, useRef } from 'react';
import { useVirtualizer } from '@tanstack/react-virtual';

import { useVisibleLogIndices } from '@/components/dashboard/logs/hooks/use-visible-log-indices';
import { useLogsFilter } from '@/components/dashboard/logs/logs-filter-context';
import { LOG_ROW_ESTIMATE_SIZE, LogRow } from '@/components/dashboard/logs/log-row';
import { LogsTableHeader } from '@/components/dashboard/logs/logs-table-header';
import { logBuffer } from '@/lib/log-buffer';

export const LogsVirtualTable = memo(function LogsVirtualTable() {
  const visibleIndices = useVisibleLogIndices();
  const { sort, setSortField } = useLogsFilter();
  const scrollRef = useRef<HTMLDivElement>(null);

  const rowVirtualizer = useVirtualizer({
    count: visibleIndices.length,
    getScrollElement: () => scrollRef.current,
    estimateSize: () => LOG_ROW_ESTIMATE_SIZE,
    overscan: 12,
    getItemKey: (index) => {
      const entryIndex = visibleIndices[index];
      if (entryIndex === undefined) {
        return index;
      }

      return logBuffer.getEntry(entryIndex)?.id ?? index;
    },
  });

  return (
    <div className="flex min-h-0 flex-1 flex-col overflow-hidden rounded-lg border bg-card">
      <LogsTableHeader sort={sort} onSortField={setSortField} />
      <div ref={scrollRef} className="min-h-0 flex-1 overflow-auto">
        {visibleIndices.length === 0 ? (
          <div className="flex h-40 items-center justify-center text-sm text-muted-foreground">
            No logs match the current filters.
          </div>
        ) : (
          <div
            className="relative w-full"
            style={{ height: `${rowVirtualizer.getTotalSize()}px` }}
          >
            {rowVirtualizer.getVirtualItems().map((virtualRow) => {
              const entryIndex = visibleIndices[virtualRow.index];
              const entry =
                entryIndex === undefined ? undefined : logBuffer.getEntry(entryIndex);

              if (!entry) {
                return null;
              }

              return (
                <LogRow
                  key={entry.id}
                  entry={entry}
                  style={{
                    height: `${virtualRow.size}px`,
                    transform: `translateY(${virtualRow.start}px)`,
                  }}
                />
              );
            })}
          </div>
        )}
      </div>
    </div>
  );
});
