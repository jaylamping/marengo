import { memo, useRef } from 'react';
import { useVirtualizer } from '@tanstack/react-virtual';

import { cn } from '@/lib/utils';

const DEFAULT_ROW_HEIGHT = 20;

type VirtualLinesListProps = {
  lines: readonly string[];
  rowHeight?: number;
  emptyMessage?: string;
  className?: string;
};

export const VirtualLinesList = memo(function VirtualLinesList({
  lines,
  rowHeight = DEFAULT_ROW_HEIGHT,
  emptyMessage = 'No lines.',
  className,
}: VirtualLinesListProps) {
  const scrollRef = useRef<HTMLDivElement>(null);

  const rowVirtualizer = useVirtualizer({
    count: lines.length,
    getScrollElement: () => scrollRef.current,
    estimateSize: () => rowHeight,
    overscan: 12,
    getItemKey: (index) => `${index}:${lines[index]?.slice(0, 48) ?? ''}`,
  });

  if (lines.length === 0) {
    return (
      <div
        className={cn(
          'flex h-40 items-center justify-center text-sm text-muted-foreground',
          className,
        )}
      >
        {emptyMessage}
      </div>
    );
  }

  return (
    <div
      ref={scrollRef}
      className={cn('min-h-0 flex-1 overflow-auto font-mono text-xs', className)}
    >
      <div
        className="relative w-full"
        style={{ height: `${rowVirtualizer.getTotalSize()}px` }}
      >
        {rowVirtualizer.getVirtualItems().map((virtualRow) => (
          <div
            key={virtualRow.key}
            className="absolute left-0 top-0 w-full truncate px-1"
            style={{
              height: `${virtualRow.size}px`,
              transform: `translateY(${virtualRow.start}px)`,
            }}
          >
            {lines[virtualRow.index]}
          </div>
        ))}
      </div>
    </div>
  );
});
