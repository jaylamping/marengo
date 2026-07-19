import { lazy, Suspense } from 'react';

import type { InventoryItem } from '@/data/robot-inventory';

import { dashboardSubsystemsClassName } from '@/components/dashboard/layout/constants';
import { InventoryTableSkeleton } from '@/components/dashboard/inventory/inventory-table-skeleton';

const InventoryDataTable = lazy(async () => {
  const module = await import('@/components/dashboard/inventory/inventory-data-table');
  return { default: module.InventoryDataTable };
});

type SubsystemsOverviewProps = {
  inventory: InventoryItem[];
};

/** Auto-loads the device table after paint — mount path no longer freezes nav. */
export function SubsystemsOverview({ inventory }: SubsystemsOverviewProps) {
  return (
    <div className={dashboardSubsystemsClassName} data-testid="subsystems-overview">
      <Suspense fallback={<InventoryTableSkeleton />}>
        <InventoryDataTable data={inventory} />
      </Suspense>
    </div>
  );
}
