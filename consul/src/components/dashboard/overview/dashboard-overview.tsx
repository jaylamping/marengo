import { lazy, Suspense } from 'react';

import { DeferredMount } from '@/components/dashboard/layout/deferred-mount';
import {
  dashboardOverviewClassName,
  dashboardOverviewHeroClassName,
} from '@/components/dashboard/layout/constants';
import { ChartSectionSkeleton } from '@/components/dashboard/overview/chart-section-skeleton';
import { OverviewPosturePanel } from '@/components/dashboard/overview/overview-posture-panel';
import { SectionCards } from '@/components/dashboard/section-cards';
import { RobotModelProvider } from '@/urdf/RobotModelContext';
import { SHOULDER_PITCH_RIGHT_ONLY_URDF } from '@/assets/urdf/shoulder-pitch-right-only';

const JointTrackingChartCard = lazy(async () => {
  const module = await import('@/components/dashboard/charts/joint-tracking-chart-card');
  return { default: module.JointTrackingChartCard };
});

export function DashboardOverview() {
  return (
    <div className={dashboardOverviewClassName}>
      <RobotModelProvider urdfXml={SHOULDER_PITCH_RIGHT_ONLY_URDF}>
        <div className={dashboardOverviewHeroClassName} data-testid="overview-hero">
          <DeferredMount fallback={<ChartSectionSkeleton />} timeoutMs={120}>
            <OverviewPosturePanel />
          </DeferredMount>
          <DeferredMount fallback={<ChartSectionSkeleton />} timeoutMs={120}>
            <Suspense fallback={<ChartSectionSkeleton />}>
              <JointTrackingChartCard />
            </Suspense>
          </DeferredMount>
        </div>
      </RobotModelProvider>

      <SectionCards />
    </div>
  );
}
