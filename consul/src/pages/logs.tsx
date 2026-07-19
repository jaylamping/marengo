import { lazy, Suspense } from 'react';

import { RouteBodyFallback } from '@/components/dashboard/layout/route-body-fallback';

const LogsOverview = lazy(async () => {
  const module = await import('@/components/dashboard/logs/logs-overview');
  return { default: module.LogsOverview };
});

/** Thin route shell — LogsOverview loads behind Suspense. */
export function LogsPage() {
  return (
    <Suspense fallback={<RouteBodyFallback />}>
      <LogsOverview />
    </Suspense>
  );
}
