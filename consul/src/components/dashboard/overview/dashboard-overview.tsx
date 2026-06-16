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
import { RobotModelProvider } from '@/urdf/RobotModelContext';
import { SHOULDER_PITCH_RIGHT_ONLY_URDF } from '@/assets/urdf/shoulder-pitch-right-only';

const JointTrackingChartCard = lazy(async () => {
  const module = await import('@/components/dashboard/charts/joint-tracking-chart-card');
  return { default: module.JointTrackingChartCard };
});

const InventoryDataTable = lazy(async () => {
  const module = await import('@/components/dashboard/inventory/inventory-data-table');
  return { default: module.InventoryDataTable };
});

const UrdfPreviewPanel = lazy(async () => {
  const module = await import('@/components/dashboard/urdf-preview/urdf-preview-panel');
  return { default: module.UrdfPreviewPanel };
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
            <RobotModelProvider urdfXml={SHOULDER_PITCH_RIGHT_ONLY_URDF}>
              <JointTrackingChartCard />
            </RobotModelProvider>
          </Suspense>
        </DeferredMount>
      </div>

      <div className={dashboardChartSectionClassName}>
        <DeferredMount fallback={<ChartSectionSkeleton />} timeoutMs={800}>
          <Suspense fallback={<ChartSectionSkeleton />}>
            <RobotModelProvider urdfXml={SHOULDER_PITCH_RIGHT_ONLY_URDF}>
              <UrdfPreviewPanel />
            </RobotModelProvider>
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
