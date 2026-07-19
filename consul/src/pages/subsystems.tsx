import { lazy, Suspense } from 'react';

import { RouteBodyFallback } from '@/components/dashboard/layout/route-body-fallback';
import { robotInventory } from '@/data/robot-inventory';
import { useLiveInventory } from '@/hooks/use-live-inventory';

const SubsystemsOverview = lazy(async () => {
  const module = await import('@/components/dashboard/subsystems/subsystems-overview');
  return { default: module.SubsystemsOverview };
});

/** Thin route shell — inventory table graph stays out of the route module. */
export function SubsystemsPage() {
  const inventory = useLiveInventory(robotInventory);

  return (
    <Suspense fallback={<RouteBodyFallback />}>
      <SubsystemsOverview inventory={inventory} />
    </Suspense>
  );
}
