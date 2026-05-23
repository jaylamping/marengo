import { DashboardLayout, DashboardOverview } from '@/components/dashboard';
import { robotInventory } from '@/data/robot-inventory';

export function DashboardPage() {
  return (
    <DashboardLayout>
      <DashboardOverview inventory={robotInventory} />
    </DashboardLayout>
  );
}
