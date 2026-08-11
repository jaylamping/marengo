import { lazy, Suspense } from 'react';

import { DeferredMount } from '@/components/dashboard/layout/deferred-mount';
import {
  dashboardOverviewCanClassName,
  dashboardOverviewClassName,
} from '@/components/dashboard/layout/constants';
import { ChartSectionSkeleton } from '@/components/dashboard/overview/chart-section-skeleton';
import { SectionCards } from '@/components/dashboard/section-cards';

const CanBusSpectrumPanel = lazy(async () => {
  const module = await import('@/components/dashboard/overview/can-bus-spectrum-panel');
  return { default: module.CanBusSpectrumPanel };
});

type DashboardOverviewProps = {
  /** When false, drop CAN spectrum polling (soft-cached off-route). */
  active?: boolean;
};

export function DashboardOverview({ active = true }: DashboardOverviewProps) {
  return (
    <div className={dashboardOverviewClassName} data-testid="dashboard-overview">
      {active ? <SectionCards /> : null}

      <div className={dashboardOverviewCanClassName} data-testid="overview-can-section">
        {active ? (
          <DeferredMount fallback={<ChartSectionSkeleton />} timeoutMs={200} strategy="idle">
            <Suspense fallback={<ChartSectionSkeleton />}>
              <CanBusSpectrumPanel active={active} />
            </Suspense>
          </DeferredMount>
        ) : (
          <ChartSectionSkeleton />
        )}
      </div>
    </div>
  );
}
