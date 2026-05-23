import { Badge } from '@/components/ui/badge';
import { DashboardCardShell } from '@/components/dashboard/cards/dashboard-card-shell';
import {
  dummyControlLoopMetrics,
  type ControlLoopMetrics,
} from '@/data/host-metrics';
import { HugeiconsIcon } from '@hugeicons/react';
import { ChartDownIcon } from '@hugeicons/core-free-icons';

type ControlLoopCardProps = {
  metrics?: ControlLoopMetrics;
};

export function ControlLoopCard({
  metrics = dummyControlLoopMetrics,
}: ControlLoopCardProps) {
  return (
    <DashboardCardShell
      description="Control Loop"
      title={`${metrics.rateHz} Hz`}
      titleClassName="text-2xl tabular-nums @[250px]/card:text-3xl"
      action={
        <Badge variant="outline">
          <HugeiconsIcon icon={ChartDownIcon} strokeWidth={2} />
          {metrics.p99Ms} ms p99
        </Badge>
      }
      footerPrimary={metrics.summary}
      footerSecondary={metrics.detail}
    />
  );
}
