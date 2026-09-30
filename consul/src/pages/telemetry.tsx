import { lazy } from 'react';

import { DeferredLazyBody } from '@/components/dashboard/layout/deferred-lazy-body';

const TelemetryOverview = lazy(async () => {
  const module = await import(
    '@/components/dashboard/telemetry/telemetry-overview'
  );
  return { default: module.TelemetryOverview };
});

/** Read-only live hardware table — commissioning lives on Hardware. */
export function TelemetryPage() {
  return (
    <DeferredLazyBody>
      <TelemetryOverview />
    </DeferredLazyBody>
  );
}
