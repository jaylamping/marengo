import { lazy, Suspense } from 'react';

import type { InventoryItem } from '@/data/robot-inventory';

import { DeferredMount } from '@/components/dashboard/layout/deferred-mount';
import { dashboardSubsystemsClassName } from '@/components/dashboard/layout/constants';
import { InventoryTableSkeleton } from '@/components/dashboard/inventory/inventory-table-skeleton';

const InventoryDataTable = lazy(async () => {
  const module = await import('@/components/dashboard/inventory/inventory-data-table');
  return { default: module.InventoryDataTable };
});

type SubsystemsOverviewProps = {
  inventory: InventoryItem[];
};

export function SubsystemsOverview({ inventory }: SubsystemsOverviewProps) {
  return (
    <div className={dashboardSubsystemsClassName} data-testid="subsystems-overview">
      <DeferredMount fallback={<InventoryTableSkeleton />} timeoutMs={2500}>
        <Suspense fallback={<InventoryTableSkeleton />}>
          <InventoryDataTable data={inventory} />
        </Suspense>
      </DeferredMount>
    </div>
  );
}
