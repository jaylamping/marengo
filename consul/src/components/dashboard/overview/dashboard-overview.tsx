import { lazy, Suspense } from 'react';

import { DeferredMount } from '@/components/dashboard/layout/deferred-mount';
import {
  dashboardChartSectionClassName,
  dashboardOverviewClassName,
} from '@/components/dashboard/layout/constants';
import { ChartSectionSkeleton } from '@/components/dashboard/overview/chart-section-skeleton';
import { SectionCards } from '@/components/dashboard/section-cards';
import { RobotModelProvider } from '@/urdf/RobotModelContext';
import { SHOULDER_PITCH_RIGHT_ONLY_URDF } from '@/assets/urdf/shoulder-pitch-right-only';

const JointTrackingChartCard = lazy(async () => {
  const module = await import('@/components/dashboard/charts/joint-tracking-chart-card');
  return { default: module.JointTrackingChartCard };
});

const UrdfPreviewPanel = lazy(async () => {
  const module = await import('@/components/dashboard/urdf-preview/urdf-preview-panel');
  return { default: module.UrdfPreviewPanel };
});

export function DashboardOverview() {
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
            <UrdfPreviewPanel />
          </Suspense>
        </DeferredMount>
      </div>
    </div>
  );
}
