import { SubsystemsOverview } from '@/components/dashboard/subsystems/subsystems-overview';
import { robotInventory } from '@/data/robot-inventory';
import { useLiveInventory } from '@/hooks/use-live-inventory';

export function SubsystemsPage() {
  const inventory = useLiveInventory(robotInventory);

  return <SubsystemsOverview inventory={inventory} />;
}
