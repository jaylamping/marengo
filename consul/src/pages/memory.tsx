import { DashboardLayout } from '@/components/dashboard/layout/dashboard-layout';
import { MemoryOverview } from '@/components/dashboard/memory/memory-overview';

export function MemoryPage() {
  return (
    <DashboardLayout>
      <MemoryOverview />
    </DashboardLayout>
  );
}
