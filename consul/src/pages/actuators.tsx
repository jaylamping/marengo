import { lazy } from 'react';

import { DeferredLazyBody } from '@/components/dashboard/layout/deferred-lazy-body';

const ActuatorsOverview = lazy(async () => {
  const module = await import('@/components/dashboard/actuators/actuators-overview');
  return { default: module.ActuatorsOverview };
});

export function ActuatorsPage() {
  return (
    <DeferredLazyBody>
      <ActuatorsOverview />
    </DeferredLazyBody>
  );
}
