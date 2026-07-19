import { lazy, Suspense } from 'react';

import { RouteBodyFallback } from '@/components/dashboard/layout/route-body-fallback';

const TestingOverview = lazy(async () => {
  const module = await import('@/components/dashboard/testing/testing-overview');
  return { default: module.TestingOverview };
});

/** Thin route shell — heavy TestingOverview loads behind Suspense. */
export function TestingPage() {
  return (
    <Suspense fallback={<RouteBodyFallback />}>
      <TestingOverview />
    </Suspense>
  );
}
