import { DashboardOverview } from '@/components/dashboard/overview/dashboard-overview';

type DashboardPageProps = {
  /** False while Overview is soft-cached off-route (pause WebGL / charts). */
  active?: boolean;
};

export function DashboardPage({ active = true }: DashboardPageProps) {
  return <DashboardOverview active={active} />;
}
