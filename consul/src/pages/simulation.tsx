import { lazy, Suspense } from 'react';

import { RouteBodyFallback } from '@/components/dashboard/layout/route-body-fallback';

const SimulationOverview = lazy(async () => {
  const module = await import('@/components/dashboard/simulation/simulation-overview');
  return { default: module.SimulationOverview };
});

/** Thin route shell — SimulationOverview (~heavy UI) loads behind Suspense. */
export function SimulationPage() {
  return (
    <Suspense fallback={<RouteBodyFallback />}>
      <SimulationOverview />
    </Suspense>
  );
}
