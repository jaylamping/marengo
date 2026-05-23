import { DashboardLayout } from '@/components/dashboard/layout/dashboard-layout';
import { LogsOverview } from '@/components/dashboard/logs/logs-overview';

export function LogsPage() {
  return (
    <DashboardLayout>
      <LogsOverview />
    </DashboardLayout>
  );
}
