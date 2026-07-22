import { useMemo } from 'react';

import { dashboardOverviewClassName } from '@/components/dashboard/layout/constants';
import { JointCard } from '@/components/dashboard/actuators/joint-card';
import { isWiredBenchJoint } from '@/data/actuator-joints';
import { robotInventory } from '@/data/robot-inventory';
import { useEnrichedInventory } from '@/hooks/use-enriched-inventory';
import { useLiveInventory } from '@/hooks/use-live-inventory';

export function ActuatorsOverview() {
  const { data: enriched = robotInventory } = useEnrichedInventory();
  const actuatorInventory = useMemo(
    () => enriched.filter((item) => item.kind === 'actuator'),
    [enriched],
  );
  const joints = useLiveInventory(actuatorInventory);

  return (
    <div
      className={`${dashboardOverviewClassName} px-4 lg:px-6`}
      data-testid="actuators-overview"
    >
      <header className="flex flex-col gap-1">
        <h2 className="text-lg font-semibold">Actuator harness</h2>
        <p className="text-sm text-muted-foreground">
          Runtime MIT tuning (debounced, live limits required) — motion controls gated until PR-5.
        </p>
      </header>

      <div className="grid gap-4 sm:grid-cols-2 xl:grid-cols-3">
        {joints.map((joint) => (
          <JointCard
            key={joint.name}
            joint={joint}
            wired={isWiredBenchJoint(joint.name)}
          />
        ))}
      </div>
    </div>
  );
}
