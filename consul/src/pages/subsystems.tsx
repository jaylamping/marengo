import { DashboardLayout } from '@/components/dashboard/layout/dashboard-layout';
import { SubsystemsOverview } from '@/components/dashboard/subsystems/subsystems-overview';
import { robotInventory } from '@/data/robot-inventory';
import { useLiveInventory } from '@/hooks/use-live-inventory';

export function SubsystemsPage() {
  const inventory = useLiveInventory(robotInventory);

  return (
    <DashboardLayout>
      <SubsystemsOverview inventory={inventory} />
    </DashboardLayout>
  );
}
