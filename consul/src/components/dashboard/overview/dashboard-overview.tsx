import { lazy, Suspense } from 'react';

import { DeferredMount } from '@/components/dashboard/layout/deferred-mount';
import {
  dashboardOverviewClassName,
  dashboardOverviewHeroClassName,
} from '@/components/dashboard/layout/constants';
import { ChartSectionSkeleton } from '@/components/dashboard/overview/chart-section-skeleton';
import { SectionCards } from '@/components/dashboard/section-cards';
import { RobotModelProvider } from '@/urdf/RobotModelContext';
import { SHOULDER_PITCH_RIGHT_ONLY_URDF } from '@/assets/urdf/shoulder-pitch-right-only';

const OverviewPosturePanel = lazy(async () => {
  const module = await import('@/components/dashboard/overview/overview-posture-panel');
  return { default: module.OverviewPosturePanel };
});

const JointTrackingChartCard = lazy(async () => {
  const module = await import('@/components/dashboard/charts/joint-tracking-chart-card');
  return { default: module.JointTrackingChartCard };
});

type DashboardOverviewProps = {
  /** When false, pause posture WebGL and drop live chart subscriptions. */
  active?: boolean;
};

export function DashboardOverview({ active = true }: DashboardOverviewProps) {
  return (
    <div className={dashboardOverviewClassName}>
      <RobotModelProvider urdfXml={SHOULDER_PITCH_RIGHT_ONLY_URDF}>
        <div className={dashboardOverviewHeroClassName} data-testid="overview-hero">
          <DeferredMount fallback={<ChartSectionSkeleton />} timeoutMs={400}>
            <Suspense fallback={<ChartSectionSkeleton />}>
              <OverviewPosturePanel active={active} />
            </Suspense>
          </DeferredMount>
          {active ? (
            <DeferredMount fallback={<ChartSectionSkeleton />} timeoutMs={200}>
              <Suspense fallback={<ChartSectionSkeleton />}>
                <JointTrackingChartCard />
              </Suspense>
            </DeferredMount>
          ) : (
            <ChartSectionSkeleton />
          )}
        </div>
      </RobotModelProvider>

      {active ? <SectionCards /> : null}
    </div>
  );
}
