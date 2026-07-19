import { lazy } from 'react';

import { DeferredLazyBody } from '@/components/dashboard/layout/deferred-lazy-body';

const LogsOverview = lazy(async () => {
  const module = await import('@/components/dashboard/logs/logs-overview');
  return { default: module.LogsOverview };
});

/** Instant route shell — skeleton paints before the heavy body mounts. */
export function LogsPage() {
  return (
    <DeferredLazyBody>
      <LogsOverview />
    </DeferredLazyBody>
  );
}
