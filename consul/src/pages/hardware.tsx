import { lazy } from 'react';

import { DeferredLazyBody } from '@/components/dashboard/layout/deferred-lazy-body';

const HardwareOverview = lazy(async () => {
  const module = await import('@/components/dashboard/hardware/hardware-overview');
  return { default: module.HardwareOverview };
});

export function HardwarePage() {
  return (
    <DeferredLazyBody>
      <HardwareOverview />
    </DeferredLazyBody>
  );
}
