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

const CanBusSpectrumPanel = lazy(async () => {
  const module = await import('@/components/dashboard/overview/can-bus-spectrum-panel');
  return { default: module.CanBusSpectrumPanel };
});

type DashboardOverviewProps = {
  /** When false, pause posture WebGL and drop CAN spectrum polling. */
  active?: boolean;
};

export function DashboardOverview({ active = true }: DashboardOverviewProps) {
  return (
    <div className={dashboardOverviewClassName}>
      <RobotModelProvider urdfXml={SHOULDER_PITCH_RIGHT_ONLY_URDF}>
        <div className={dashboardOverviewHeroClassName} data-testid="overview-hero">
          <DeferredMount fallback={<ChartSectionSkeleton />} timeoutMs={400} strategy="idle">
            <Suspense fallback={<ChartSectionSkeleton />}>
              <OverviewPosturePanel active={active} />
            </Suspense>
          </DeferredMount>
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
      </RobotModelProvider>

      {active ? <SectionCards /> : null}
    </div>
  );
}
