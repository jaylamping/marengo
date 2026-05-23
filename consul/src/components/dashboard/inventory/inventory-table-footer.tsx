import type { InventoryTable } from '@/components/dashboard/inventory/utils';

type InventoryTableFooterProps = {
  table: InventoryTable;
  groupCount: number;
  expandedGroupCount: number;
  totalDeviceCount: number;
};

export function InventoryTableFooter({
  table,
  groupCount,
  expandedGroupCount,
  totalDeviceCount,
}: InventoryTableFooterProps) {
  return (
    <div className="flex items-center justify-between text-sm text-muted-foreground">
      <div>
        {table.getFilteredSelectedRowModel().rows.length} of{' '}
        {table.getFilteredRowModel().rows.length} selected · {groupCount} groups ·{' '}
        {expandedGroupCount} expanded
      </div>
      <div>{totalDeviceCount} total devices (dummy inventory)</div>
    </div>
  );
}
