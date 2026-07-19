import { flexRender, type Row } from '@tanstack/react-table';

import type { InventoryRow } from '@/components/dashboard/inventory/types';
import { TableCell, TableRow } from '@/components/ui/table';

type InventoryTableRowProps = {
  row: Row<InventoryRow>;
};

/** Plain table row — no per-row DnD registration. */
export function InventoryTableRow({ row }: InventoryTableRowProps) {
  return (
    <TableRow data-state={row.getIsSelected() && 'selected'}>
      {row.getVisibleCells().map((cell) => (
        <TableCell key={cell.id}>
          {flexRender(cell.column.columnDef.cell, cell.getContext())}
        </TableCell>
      ))}
    </TableRow>
  );
}
