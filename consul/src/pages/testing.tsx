import { lazy } from 'react';

import { DeferredLazyBody } from '@/components/dashboard/layout/deferred-lazy-body';

const TestingOverview = lazy(async () => {
  const module = await import('@/components/dashboard/testing/testing-overview');
  return { default: module.TestingOverview };
});

/** Instant route shell — skeleton paints before the heavy body mounts. */
export function TestingPage() {
  return (
    <DeferredLazyBody>
      <TestingOverview />
    </DeferredLazyBody>
  );
}
