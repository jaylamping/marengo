import { lazy, Suspense } from 'react';

import { RouteBodyFallback } from '@/components/dashboard/layout/route-body-fallback';

const ActuatorsOverview = lazy(async () => {
  const module = await import('@/components/dashboard/actuators/actuators-overview');
  return { default: module.ActuatorsOverview };
});

export function ActuatorsPage() {
  return (
    <Suspense fallback={<RouteBodyFallback />}>
      <ActuatorsOverview />
    </Suspense>
  );
}
