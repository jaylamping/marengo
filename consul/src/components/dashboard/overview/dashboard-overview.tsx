import type { InventoryItem } from '@/data/robot-inventory';

import { JointTrackingChartCard } from '@/components/dashboard/charts/joint-tracking-chart-card';
import {
  dashboardChartSectionClassName,
  dashboardOverviewClassName,
} from '@/components/dashboard/layout/constants';
import { InventoryDataTable } from '@/components/dashboard/inventory/inventory-data-table';
import { SectionCards } from '@/components/dashboard/section-cards';

type DashboardOverviewProps = {
  inventory: InventoryItem[];
};

export function DashboardOverview({ inventory }: DashboardOverviewProps) {
  return (
    <div className={dashboardOverviewClassName}>
      <SectionCards />
      <div className={dashboardChartSectionClassName}>
        <JointTrackingChartCard />
      </div>
      <InventoryDataTable data={inventory} />
    </div>
  );
}
