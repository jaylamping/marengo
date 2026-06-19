import { DashboardLayout } from '@/components/dashboard/layout/dashboard-layout';
import { ActuatorsOverview } from '@/components/dashboard/actuators/actuators-overview';

export function ActuatorsPage() {
  return (
    <DashboardLayout>
      <ActuatorsOverview />
    </DashboardLayout>
  );
}
