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
    <div className="flex items-center justify-between font-mono text-[10px] tracking-[0.14em] text-muted-foreground uppercase">
      <div>
        {table.getFilteredSelectedRowModel().rows.length} of{' '}
        {table.getFilteredRowModel().rows.length} selected · {groupCount} groups ·{' '}
        {expandedGroupCount} expanded
      </div>
      <div>{totalDeviceCount} devices</div>
    </div>
  );
}
