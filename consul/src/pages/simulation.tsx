import { lazy } from 'react';

import { DeferredLazyBody } from '@/components/dashboard/layout/deferred-lazy-body';

const SimulationOverview = lazy(async () => {
  const module = await import('@/components/dashboard/simulation/simulation-overview');
  return { default: module.SimulationOverview };
});

/** Instant route shell — skeleton paints before the heavy body mounts. */
export function SimulationPage() {
  return (
    <DeferredLazyBody>
      <SimulationOverview />
    </DeferredLazyBody>
  );
}
