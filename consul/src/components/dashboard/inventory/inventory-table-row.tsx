import { flexRender, type Row } from '@tanstack/react-table';

import type { InventoryColumnMeta } from '@/components/dashboard/inventory/inventory-columns';
import type { InventoryRow } from '@/components/dashboard/inventory/types';
import { cn } from '@/lib/utils';
import { TableCell, TableRow } from '@/components/ui/table';

type InventoryTableRowProps = {
  row: Row<InventoryRow>;
};

function columnMetaClassName(meta: unknown): string | undefined {
  return (meta as InventoryColumnMeta | undefined)?.className;
}

function columnSize(column: {
  getSize?: () => number;
  columnDef: { size?: number };
}): number | undefined {
  if (typeof column.getSize === 'function') {
    return column.getSize();
  }
  return column.columnDef.size;
}

/** Plain table row — no per-row DnD registration. */
export function InventoryTableRow({ row }: InventoryTableRowProps) {
  return (
    <TableRow data-state={row.getIsSelected() && 'selected'}>
      {row.getVisibleCells().map((cell) => (
        <TableCell
          key={cell.id}
          className={cn(columnMetaClassName(cell.column.columnDef.meta))}
          style={{ width: columnSize(cell.column) }}
        >
          {flexRender(cell.column.columnDef.cell, cell.getContext())}
        </TableCell>
      ))}
    </TableRow>
  );
}
