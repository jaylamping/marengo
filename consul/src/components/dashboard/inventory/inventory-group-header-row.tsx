import type { InventoryGroup } from '@/data/robot-inventory';

import { cn } from '@/lib/utils';
import { Badge } from '@/components/ui/badge';
import { TableCell, TableRow } from '@/components/ui/table';
import { HugeiconsIcon } from '@hugeicons/react';
import { ArrowDown01Icon } from '@hugeicons/core-free-icons';

type InventoryGroupHeaderRowProps = {
  group: InventoryGroup;
  label: string;
  rowCount: number;
  columnCount: number;
  isCollapsed: boolean;
  onToggle: () => void;
};

export function InventoryGroupHeaderRow({
  group,
  label,
  rowCount,
  columnCount,
  isCollapsed,
  onToggle,
}: InventoryGroupHeaderRowProps) {
  return (
    <TableRow className="bg-surface-2/60 hover:bg-surface-2">
      <TableCell colSpan={columnCount} className="p-0">
        <button
          type="button"
          aria-expanded={!isCollapsed}
          aria-controls={`inventory-group-${group}`}
          onClick={onToggle}
          className="flex w-full items-center gap-2 px-4 py-2 text-left font-mono text-[10px] font-semibold tracking-[0.14em] text-muted-foreground uppercase"
        >
          <HugeiconsIcon
            icon={ArrowDown01Icon}
            strokeWidth={2}
            className={cn(
              'size-3.5 shrink-0 transition-transform',
              isCollapsed && '-rotate-90',
            )}
          />
          <span>{label}</span>
          <Badge variant="secondary" className="ml-1 font-mono text-[10px]">
            {rowCount}
          </Badge>
        </button>
      </TableCell>
    </TableRow>
  );
}
