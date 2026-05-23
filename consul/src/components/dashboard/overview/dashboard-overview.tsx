import { lazy, Suspense } from 'react';

import type { InventoryItem } from '@/data/robot-inventory';

import { DeferredMount } from '@/components/dashboard/layout/deferred-mount';
import {
  dashboardChartSectionClassName,
  dashboardOverviewClassName,
} from '@/components/dashboard/layout/constants';
import { InventoryTableSkeleton } from '@/components/dashboard/inventory/inventory-table-skeleton';
import { ChartSectionSkeleton } from '@/components/dashboard/overview/chart-section-skeleton';
import { SectionCards } from '@/components/dashboard/section-cards';

const JointTrackingChartCard = lazy(async () => {
  const module = await import('@/components/dashboard/charts/joint-tracking-chart-card');
  return { default: module.JointTrackingChartCard };
});

const InventoryDataTable = lazy(async () => {
  const module = await import('@/components/dashboard/inventory/inventory-data-table');
  return { default: module.InventoryDataTable };
});

type DashboardOverviewProps = {
  inventory: InventoryItem[];
};

export function DashboardOverview({ inventory }: DashboardOverviewProps) {
  return (
    <div className={dashboardOverviewClassName}>
      <SectionCards />

      <div className={dashboardChartSectionClassName}>
        <DeferredMount fallback={<ChartSectionSkeleton />} timeoutMs={800}>
          <Suspense fallback={<ChartSectionSkeleton />}>
            <JointTrackingChartCard />
          </Suspense>
        </DeferredMount>
      </div>

      <DeferredMount fallback={<InventoryTableSkeleton />} timeoutMs={1500}>
        <Suspense fallback={<InventoryTableSkeleton />}>
          <InventoryDataTable data={inventory} />
        </Suspense>
      </DeferredMount>
    </div>
  );
}
