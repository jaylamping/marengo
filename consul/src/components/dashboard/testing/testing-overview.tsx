import {
  dashboardOverviewClassName,
  dashboardPanelPointerClassName,
} from '@/components/dashboard/layout/constants';
import { ActuatorMultiSelect } from '@/components/dashboard/testing/actuator-multi-select';
import { PresetGroupButtons } from '@/components/dashboard/testing/preset-group-buttons';
import { HoldAtControls } from '@/components/dashboard/testing/hold-at-controls';
import { PidSliderPanel } from '@/components/dashboard/testing/pid-slider-panel';
import { TelemetryGaugeGrid } from '@/components/dashboard/testing/telemetry-gauge-grid';
import { EnableDisableButtons } from '@/components/dashboard/testing/enable-disable-buttons';
import { EStopButton } from '@/components/dashboard/testing/e-stop-button';

export function TestingOverview() {
  return (
    <div
      className={`${dashboardOverviewClassName} ${dashboardPanelPointerClassName} px-4 lg:px-6`}
    >
      <div className="sticky top-0 z-10 bg-background/80 backdrop-blur p-4 flex gap-4 items-center border-b">
        <EnableDisableButtons />
        <div className="flex-1" />
        <EStopButton />
      </div>

      <div className="grid grid-cols-1 lg:grid-cols-3 gap-4">
        <div className="lg:col-span-2 flex flex-col gap-4">
          <ActuatorMultiSelect />
          <PresetGroupButtons />
          <HoldAtControls />
          <PidSliderPanel />
        </div>
        <div className="lg:col-span-1">
          <TelemetryGaugeGrid />
        </div>
      </div>
    </div>
  );
}
