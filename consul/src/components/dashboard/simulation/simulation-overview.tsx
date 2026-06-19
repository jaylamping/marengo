import { dashboardOverviewClassName } from '@/components/dashboard/layout/constants';
import { simOverviewShellClassName } from '@/components/dashboard/simulation/constants';
import { SimControlBar } from '@/components/dashboard/simulation/sim-control-bar';
import { SimEventLog } from '@/components/dashboard/simulation/sim-event-log';
import { SimRuntimeMetricsCard } from '@/components/dashboard/simulation/sim-runtime-metrics-card';
import { SimScenariosTable } from '@/components/dashboard/simulation/sim-scenarios-table';
import { SimSessionCard } from '@/components/dashboard/simulation/sim-session-card';
import { SimViewportPlaceholder } from '@/components/dashboard/simulation/sim-viewport-placeholder';
import {
  dummySimEvents,
  dummySimRuntimeMetrics,
  dummySimScenarios,
  dummySimSession,
} from '@/data/simulation';

export function SimulationOverview() {
  return (
    <div
      className={`${dashboardOverviewClassName} ${simOverviewShellClassName} px-4 lg:px-6`}
      data-testid="simulation-overview"
    >
      <SimControlBar sessionState={dummySimSession.state} />

      <div className="grid gap-4 lg:grid-cols-3">
        <div className="lg:col-span-2">
          <SimViewportPlaceholder />
        </div>
        <div className="flex flex-col gap-4">
          <SimSessionCard session={dummySimSession} />
          <SimRuntimeMetricsCard metrics={dummySimRuntimeMetrics} />
        </div>
      </div>

      <SimScenariosTable scenarios={dummySimScenarios} />
      <SimEventLog events={dummySimEvents} />
    </div>
  );
}
