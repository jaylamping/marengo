import { DashboardCardShell } from '@/components/dashboard/cards/dashboard-card-shell';
import { MetricGrid } from '@/components/dashboard/metrics/metric-grid';
import { MetricItem } from '@/components/dashboard/metrics/metric-item';
import { StatusBadge } from '@/components/dashboard/metrics/status-badge';
import {
  dummyPiHostMetrics,
  type PiHostMetrics,
} from '@/data/host-metrics';
import {
  formatLoad,
  formatPercent,
  formatRamUsage,
  formatTempC,
} from '@/lib/format';

type PiHostCardProps = {
  metrics?: PiHostMetrics;
};

export function PiHostCard({ metrics = dummyPiHostMetrics }: PiHostCardProps) {
  return (
    <DashboardCardShell
      description="Pi 5 · onboard"
      title={metrics.hostname}
      action={
        <StatusBadge
          label={metrics.throttled ? 'throttled' : 'healthy'}
          tone={metrics.throttled ? 'warning' : 'healthy'}
        />
      }
      content={
        <MetricGrid>
          <MetricItem label="CPU" value={formatPercent(metrics.cpuPercent)} />
          <MetricItem
            label="RAM"
            value={formatRamUsage(metrics.ramUsedGb, metrics.ramTotalGb)}
            valueClassName="text-sm"
          />
          <MetricItem label="Temp" value={formatTempC(metrics.tempC)} />
          <MetricItem label="Load (1m)" value={formatLoad(metrics.load1m)} />
        </MetricGrid>
      }
      footerPrimary={`Uptime ${metrics.uptime}`}
      footerSecondary={metrics.servicesLabel}
    />
  );
}
