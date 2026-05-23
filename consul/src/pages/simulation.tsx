import { DashboardLayout } from '@/components/dashboard/layout/dashboard-layout';
import { SimulationOverview } from '@/components/dashboard/simulation/simulation-overview';

export function SimulationPage() {
  return (
    <DashboardLayout>
      <SimulationOverview />
    </DashboardLayout>
  );
}
