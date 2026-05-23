import { useSortable } from '@dnd-kit/sortable';

import { Button } from '@/components/ui/button';
import { HugeiconsIcon } from '@hugeicons/react';
import { DragDropVerticalIcon } from '@hugeicons/core-free-icons';

type DragHandleProps = {
  id: number;
};

export function DragHandle({ id }: DragHandleProps) {
  const { attributes, listeners } = useSortable({ id });

  return (
    <Button
      {...attributes}
      {...listeners}
      variant="ghost"
      size="icon"
      className="size-7 text-muted-foreground hover:bg-transparent"
    >
      <HugeiconsIcon
        icon={DragDropVerticalIcon}
        strokeWidth={2}
        className="size-3 text-muted-foreground"
      />
      <span className="sr-only">Drag to reorder</span>
    </Button>
  );
}
