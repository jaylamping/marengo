import { DashboardCardShell } from '@/components/dashboard/cards/dashboard-card-shell';
import { MetricGrid } from '@/components/dashboard/metrics/metric-grid';
import { MetricItem } from '@/components/dashboard/metrics/metric-item';
import { StatusBadge } from '@/components/dashboard/metrics/status-badge';
import {
  dummyJetsonHostMetrics,
  type JetsonHostMetrics,
} from '@/data/host-metrics';
import {
  computeRamUsagePercent,
  formatLoad,
  formatPercent,
  formatRamUsage,
  formatTempC,
} from '@/lib/format';

type JetsonHostCardProps = {
  metrics?: JetsonHostMetrics;
};

export function JetsonHostCard({
  metrics = dummyJetsonHostMetrics,
}: JetsonHostCardProps) {
  return (
    <DashboardCardShell
      description="Jetson · perception"
      title={metrics.hostname}
      action={
        <StatusBadge
          label={metrics.online ? 'online' : 'offline'}
          tone={metrics.online ? 'healthy' : 'muted'}
        />
      }
      content={
        <MetricGrid>
          <MetricItem
            label="CPU"
            value={formatPercent(metrics.cpuPercent)}
            usagePercent={metrics.cpuPercent}
          />
          <MetricItem
            label="RAM"
            value={formatRamUsage(metrics.ramUsedGb, metrics.ramTotalGb)}
            valueClassName="text-xs"
            usagePercent={computeRamUsagePercent(
              metrics.ramUsedGb,
              metrics.ramTotalGb,
            )}
          />
          <MetricItem
            label="GPU"
            value={formatPercent(metrics.gpuPercent)}
            usagePercent={metrics.gpuPercent}
          />
          <MetricItem label="Temp" value={formatTempC(metrics.tempC)} />
        </MetricGrid>
      }
      footerPrimary={`Uptime ${metrics.uptime} · ${metrics.powerMode} · load ${formatLoad(metrics.load1m)}`}
      footerSecondary={`${metrics.servicesLabel} · ${metrics.chappeRttMs.toFixed(1)} ms`}
    />
  );
}
