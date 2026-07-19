import { lazy } from 'react';

import { DeferredLazyBody } from '@/components/dashboard/layout/deferred-lazy-body';
import { robotInventory } from '@/data/robot-inventory';
import { useEnrichedInventory } from '@/hooks/use-enriched-inventory';

const SubsystemsOverview = lazy(async () => {
  const module = await import('@/components/dashboard/subsystems/subsystems-overview');
  return { default: module.SubsystemsOverview };
});

/**
 * Instant route shell — enriched inventory from the shared config query (SWR + persist).
 * No live overlay: config updates flow into the table; Chappe ticks do not.
 */
export function SubsystemsPage() {
  const { data: inventory = robotInventory } = useEnrichedInventory();

  return (
    <DeferredLazyBody>
      <SubsystemsOverview inventory={inventory} />
    </DeferredLazyBody>
  );
}
