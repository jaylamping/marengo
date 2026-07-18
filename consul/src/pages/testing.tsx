import { DashboardLayout } from '@/components/dashboard/layout/dashboard-layout';
import { TestingOverview } from '@/components/dashboard/testing/testing-overview';

export function TestingPage() {
  return (
    <DashboardLayout>
      <TestingOverview />
    </DashboardLayout>
  );
}