import { useInventoryDetail } from '@/components/dashboard/inventory/inventory-detail-context';
import type { InventoryRow } from '@/components/dashboard/inventory/types';
import { Button } from '@/components/ui/button';

type InventoryNameCellProps = {
  item: InventoryRow;
};

/** Lightweight name cell — detail drawer mounts once at table level when opened. */
export function InventoryNameCell({ item }: InventoryNameCellProps) {
  const { openItem } = useInventoryDetail();

  return (
    <div className="min-w-[10rem]">
      <Button
        type="button"
        variant="link"
        className="w-fit px-0 text-left font-mono text-sm text-foreground"
        onClick={() => openItem(item)}
      >
        {item.name}
      </Button>
      <div className="mt-0.5 font-mono text-[10px] text-muted-foreground">
        {item.node}
      </div>
    </div>
  );
}
