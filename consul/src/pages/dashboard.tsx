import { DashboardLayout, DashboardOverview } from '@/components/dashboard';
import { robotInventory } from '@/data/robot-inventory';
import { useLiveInventory } from '@/hooks/use-live-inventory';

export function DashboardPage() {
  const inventory = useLiveInventory(robotInventory);

  return (
    <DashboardLayout>
      <DashboardOverview inventory={inventory} />
    </DashboardLayout>
  );
}
