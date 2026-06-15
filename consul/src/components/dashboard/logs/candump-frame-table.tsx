import { memo, useRef } from 'react';
import { useVirtualizer } from '@tanstack/react-virtual';

import type { CandumpFrameDto } from '@/lib/log-api';

const ROW_HEIGHT = 28;

type Props = {
  frames: CandumpFrameDto[];
  total: number;
  offset: number;
  pageSize: number;
  onPage: (offset: number) => void;
};

export const CandumpFrameTable = memo(function CandumpFrameTable({
  frames,
  total,
  offset,
  pageSize,
  onPage,
}: Props) {
  const scrollRef = useRef<HTMLDivElement>(null);

  const rowVirtualizer = useVirtualizer({
    count: frames.length,
    getScrollElement: () => scrollRef.current,
    estimateSize: () => ROW_HEIGHT,
    overscan: 8,
    getItemKey: (index) => frames[index]?.line_no ?? index,
  });

  return (
    <div className="flex min-h-0 flex-1 flex-col gap-3">
      <div className="flex items-center justify-between text-sm text-muted-foreground">
        <span>
          {total} frames · showing {offset + 1}–{Math.min(offset + pageSize, total)}
        </span>
        <div className="flex gap-2">
          <button
            type="button"
            className="rounded border px-2 py-1 disabled:opacity-40"
            disabled={offset <= 0}
            onClick={() => onPage(Math.max(0, offset - pageSize))}
          >
            Prev
          </button>
          <button
            type="button"
            className="rounded border px-2 py-1 disabled:opacity-40"
            disabled={offset + pageSize >= total}
            onClick={() => onPage(offset + pageSize)}
          >
            Next
          </button>
        </div>
      </div>
      <div className="flex min-h-0 flex-1 flex-col overflow-hidden rounded-lg border">
        <div className="grid shrink-0 grid-cols-[minmax(88px,0.2fr)_minmax(48px,0.12fr)_minmax(72px,0.15fr)_minmax(0,1fr)] border-b bg-card px-2 py-2 text-left text-xs text-muted-foreground">
          <span>Δt</span>
          <span>if</span>
          <span>id</span>
          <span>data</span>
        </div>
        <div ref={scrollRef} className="min-h-0 flex-1 overflow-auto">
          {frames.length === 0 ? (
            <div className="flex h-32 items-center justify-center text-sm text-muted-foreground">
              No frames on this page.
            </div>
          ) : (
            <div
              className="relative w-full"
              style={{ height: `${rowVirtualizer.getTotalSize()}px` }}
            >
              {rowVirtualizer.getVirtualItems().map((virtualRow) => {
                const frame = frames[virtualRow.index];
                if (!frame) {
                  return null;
                }
                return (
                  <div
                    key={virtualRow.key}
                    className="absolute left-0 top-0 grid w-full grid-cols-[minmax(88px,0.2fr)_minmax(48px,0.12fr)_minmax(72px,0.15fr)_minmax(0,1fr)] border-b px-2 py-1 font-mono text-xs"
                    style={{
                      height: `${virtualRow.size}px`,
                      transform: `translateY(${virtualRow.start}px)`,
                    }}
                  >
                    <span className="truncate tabular-nums">{frame.delta_s.toFixed(6)}</span>
                    <span className="truncate">{frame.interface}</span>
                    <span className="truncate">{frame.can_id}</span>
                    <span className="truncate">{frame.data}</span>
                  </div>
                );
              })}
            </div>
          )}
        </div>
      </div>
    </div>
  );
});
