import { lazy, Suspense } from 'react';

import {
  dashboardOverviewClassName,
  dashboardPanelPointerClassName,
} from '@/components/dashboard/layout/constants';
import { DeferredMount } from '@/components/dashboard/layout/deferred-mount';
import { ActuatorMultiSelect } from '@/components/dashboard/testing/actuator-multi-select';
import { PresetGroupButtons } from '@/components/dashboard/testing/preset-group-buttons';
import { HoldAtControls } from '@/components/dashboard/testing/hold-at-controls';
import { PidSliderPanel } from '@/components/dashboard/testing/pid-slider-panel';
import { TelemetryGaugeGrid } from '@/components/dashboard/testing/telemetry-gauge-grid';
import { EnableDisableButtons } from '@/components/dashboard/testing/enable-disable-buttons';
import { EStopButton } from '@/components/dashboard/testing/e-stop-button';
import { Tabs, TabsContent, TabsList, TabsTrigger } from '@/components/ui/tabs';
import { Separator } from '@/components/ui/separator';
import { Skeleton } from '@/components/ui/skeleton';

const CompoundTestPanel = lazy(async () => {
  const module = await import('@/components/dashboard/testing/compound-test-panel');
  return { default: module.CompoundTestPanel };
});

const TeachRecordPanel = lazy(async () => {
  const module = await import('@/components/dashboard/testing/teach-record-panel');
  return { default: module.TeachRecordPanel };
});

function TestingBodySkeleton() {
  return (
    <div className="grid grid-cols-1 gap-6 lg:grid-cols-12" data-testid="testing-body-skeleton">
      <div className="flex flex-col gap-4 lg:col-span-8">
        <Skeleton className="h-10 w-full" />
        <Skeleton className="h-24 w-full" />
        <Skeleton className="h-40 w-full" />
      </div>
      <Skeleton className="h-64 w-full lg:col-span-4" />
    </div>
  );
}

export function TestingOverview() {
  return (
    <div
      className={`${dashboardOverviewClassName} ${dashboardPanelPointerClassName} px-4 lg:px-6 pb-8`}
    >
      <div className="sticky top-0 z-10 bg-surface-1 p-4 flex gap-4 items-center border-b border-line mb-6">
        <EnableDisableButtons />
        <div className="flex-1" />
        <div className="w-48">
          <EStopButton />
        </div>
      </div>

      <DeferredMount fallback={<TestingBodySkeleton />} timeoutMs={120} strategy="idle">
        <Tabs defaultValue="manual" className="w-full">
          <TabsList className="grid w-full max-w-xl grid-cols-3 mb-6">
            <TabsTrigger value="manual">Manual Testing</TabsTrigger>
            <TabsTrigger value="compound">Compound Tests</TabsTrigger>
            <TabsTrigger value="teach">Teach Record</TabsTrigger>
          </TabsList>

          <TabsContent value="manual" className="mt-0">
            <div className="grid grid-cols-1 lg:grid-cols-12 gap-6">
              <div className="lg:col-span-8 flex flex-col gap-6">
                <ActuatorMultiSelect />
                <PresetGroupButtons />
                <Separator />
                <HoldAtControls />
                <PidSliderPanel />
              </div>
              <div className="lg:col-span-4">
                <TelemetryGaugeGrid />
              </div>
            </div>
          </TabsContent>

          {/* forceMount: keep Wave runner alive across tab switches so Record cannot
              race a Pi-side PositionWave after Consul clears isRunning on unmount. */}
          <TabsContent value="compound" forceMount className="mt-0 data-[state=inactive]:hidden">
            <div className="grid grid-cols-1 lg:grid-cols-12 gap-6">
              <div className="lg:col-span-8 flex flex-col gap-6">
                <Suspense fallback={<Skeleton className="h-64 w-full" />}>
                  <CompoundTestPanel />
                </Suspense>
                <Separator />
                <PidSliderPanel />
              </div>
              <div className="lg:col-span-4">
                <TelemetryGaugeGrid />
              </div>
            </div>
          </TabsContent>

          <TabsContent value="teach" forceMount className="mt-0 data-[state=inactive]:hidden">
            <div className="grid grid-cols-1 lg:grid-cols-12 gap-6">
              <div className="lg:col-span-8 flex flex-col gap-6">
                <Suspense fallback={<Skeleton className="h-64 w-full" />}>
                  <TeachRecordPanel />
                </Suspense>
              </div>
              <div className="lg:col-span-4">
                <TelemetryGaugeGrid />
              </div>
            </div>
          </TabsContent>
        </Tabs>
      </DeferredMount>
    </div>
  );
}
