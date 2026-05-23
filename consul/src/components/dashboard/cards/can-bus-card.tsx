import { Badge } from '@/components/ui/badge';
import { DashboardCardShell } from '@/components/dashboard/cards/dashboard-card-shell';
import {
  dummyCanBusMetrics,
  type CanBusMetrics,
} from '@/data/host-metrics';

type CanBusCardProps = {
  metrics?: CanBusMetrics;
};

export function CanBusCard({ metrics = dummyCanBusMetrics }: CanBusCardProps) {
  return (
    <DashboardCardShell
      description="CAN Bus"
      title={`${metrics.interface} ${metrics.status}`}
      titleClassName="text-2xl tabular-nums @[250px]/card:text-3xl"
      action={
        <Badge variant="outline">{metrics.nodeCount} nodes</Badge>
      }
      footerPrimary={`${metrics.avgRttMs} ms avg RTT`}
      footerSecondary={metrics.detail}
    />
  );
}
