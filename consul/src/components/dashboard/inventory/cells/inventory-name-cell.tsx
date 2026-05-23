import { InventoryRowDrawer } from '@/components/dashboard/inventory/inventory-row-drawer';
import type { InventoryRow } from '@/components/dashboard/inventory/types';

type InventoryNameCellProps = {
  item: InventoryRow;
};

export function InventoryNameCell({ item }: InventoryNameCellProps) {
  return (
    <div className="min-w-[10rem]">
      <InventoryRowDrawer item={item} />
      <div className="mt-0.5 font-mono text-[10px] text-muted-foreground">
        {item.node}
      </div>
    </div>
  );
}
