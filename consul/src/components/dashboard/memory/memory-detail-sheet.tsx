import { memorySheetContentClassName } from '@/components/dashboard/memory/constants';
import {
  Sheet,
  SheetContent,
  SheetDescription,
  SheetHeader,
  SheetTitle,
} from '@/components/ui/sheet';
import type { Mem0HistoryEvent, Mem0Memory } from '@/lib/mem0-config';
import { cn } from '@/lib/utils';

type MemoryDetailSheetProps = {
  memory: Mem0Memory | null;
  history: Mem0HistoryEvent[];
  open: boolean;
  onOpenChange: (open: boolean) => void;
};

export function MemoryDetailSheet({
  memory,
  history,
  open,
  onOpenChange,
}: MemoryDetailSheetProps) {
  return (
    <Sheet open={open} onOpenChange={onOpenChange}>
      <SheetContent className={cn('w-full overflow-y-auto sm:max-w-xl', memorySheetContentClassName)}>
        <SheetHeader>
          <SheetTitle>{memory?.topicKey || memory?.id || 'Memory'}</SheetTitle>
          <SheetDescription>
            {memory?.updated_at ?? memory?.created_at ?? 'read-only · mem0 history'}
          </SheetDescription>
        </SheetHeader>
        {memory ? (
          <div className="mt-4 space-y-4">
            <pre className="whitespace-pre-wrap rounded-lg border bg-muted/20 p-3 text-xs">
              {memory.memory}
            </pre>
            <div>
              <h3 className="mb-2 text-sm font-semibold">History</h3>
              {history.length === 0 ? (
                <p className="text-sm text-muted-foreground">No revision history returned.</p>
              ) : (
                <ul className="space-y-2 text-sm">
                  {history.map((event, index) => (
                    <li key={event.id ?? index} className="rounded border p-2">
                      <div className="text-xs text-muted-foreground">
                        {event.event ?? 'UPDATE'} · {event.created_at ?? '?'}
                      </div>
                      <p className="mt-1 line-clamp-4">{event.new_memory ?? event.old_memory ?? ''}</p>
                    </li>
                  ))}
                </ul>
              )}
            </div>
          </div>
        ) : null}
      </SheetContent>
    </Sheet>
  );
}
