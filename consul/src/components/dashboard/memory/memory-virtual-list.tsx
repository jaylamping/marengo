import { useVirtualizer } from '@tanstack/react-virtual';
import { useRef } from 'react';

import { memoryListShellClassName } from '@/components/dashboard/memory/constants';
import type { Mem0Memory } from '@/lib/mem0-config';
import { cn } from '@/lib/utils';

type MemoryVirtualListProps = {
  memories: Mem0Memory[];
  selectedId: string | null;
  onSelect: (memory: Mem0Memory) => void;
};

export function MemoryVirtualList({ memories, selectedId, onSelect }: MemoryVirtualListProps) {
  const parentRef = useRef<HTMLDivElement>(null);
  const virtualizer = useVirtualizer({
    count: memories.length,
    getScrollElement: () => parentRef.current,
    estimateSize: () => 72,
    overscan: 8,
  });

  if (memories.length === 0) {
    return (
      <div className={cn(memoryListShellClassName, 'border-dashed p-6 text-sm text-muted-foreground')}>
        No memories in this namespace.
      </div>
    );
  }

  return (
    <div ref={parentRef} className={memoryListShellClassName} data-testid="memory-list-shell">
      <div style={{ height: `${virtualizer.getTotalSize()}px`, position: 'relative' }}>
        {virtualizer.getVirtualItems().map((item) => {
          const memory = memories[item.index];
          const preview = memory.memory.slice(0, 120).replace(/\n/g, ' ');
          return (
            <button
              key={memory.id}
              type="button"
              className={cn(
                'absolute inset-x-0 flex w-full flex-col gap-1 border-b bg-card/95 px-3 py-2 text-left text-sm hover:bg-muted/40',
                selectedId === memory.id && 'bg-muted',
              )}
              style={{ transform: `translateY(${item.start}px)`, height: `${item.size}px` }}
              onClick={() => onSelect(memory)}
            >
              <div className="flex items-center justify-between gap-2">
                <span className="font-medium truncate">{memory.topicKey || memory.id}</span>
                <span className="text-xs text-muted-foreground shrink-0">{memory.namespace}</span>
              </div>
              <span className="text-xs text-muted-foreground line-clamp-2">{preview}</span>
            </button>
          );
        })}
      </div>
    </div>
  );
}
