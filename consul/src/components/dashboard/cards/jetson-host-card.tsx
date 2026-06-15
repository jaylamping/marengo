import { DashboardCardShell } from '@/components/dashboard/cards/dashboard-card-shell';
import { HostDebugTooltip } from '@/components/dashboard/metrics/host-debug-tooltip';
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
import { isChappeLive } from '@/lib/chappe-config';
import { hostDebugLinesFromMetrics } from '@/lib/host-debug-info';
import {
  hostMetricsStale,
  useHostMetricsStore,
} from '@/state/hostMetricsStore';

function formatUptime(seconds: bigint | number): string {
  const total = Number(seconds);
  const hours = Math.floor(total / 3600);
  const minutes = Math.floor((total % 3600) / 60);
  if (hours > 0) {
    return `${hours}h ${minutes}m`;
  }
  return `${minutes}m`;
}

function liveJetsonMetrics(
  metrics: ReturnType<typeof useHostMetricsStore.getState>['jetsonMetrics'],
): JetsonHostMetrics | null {
  if (!metrics) {
    return null;
  }
  const mem = metrics.memory;
  return {
    hostname: metrics.hostname || 'marengo-jetson',
    cpuPercent: metrics.cpu?.usagePercent ?? 0,
    ramUsedGb: mem ? Number(mem.usedBytes) / 1024 ** 3 : 0,
    ramTotalGb: mem ? Number(mem.totalBytes) / 1024 ** 3 : 0,
    gpuPercent:
      metrics.platform.case === 'jetson'
        ? metrics.platform.value.gpuUsagePercent
        : 0,
    tempC: metrics.thermal?.cpuCelsius ?? 0,
    load1m: metrics.load?.load1m ?? 0,
    uptime: formatUptime(metrics.uptimeSec),
    powerMode:
      metrics.platform.case === 'jetson'
        ? metrics.platform.value.powerMode || '—'
        : '—',
    chappeRttMs:
      metrics.platform.case === 'jetson'
        ? metrics.platform.value.chappeRttMs
        : 0,
    online:
      metrics.platform.case === 'jetson'
        ? metrics.platform.value.chappeConnected
        : false,
  };
}

type JetsonHostCardProps = {
  metrics?: JetsonHostMetrics;
};

export function JetsonHostCard({
  metrics: metricsProp,
}: JetsonHostCardProps) {
  const liveMetrics = useHostMetricsStore((s) => s.jetsonMetrics);
  const liveUpdatedAt = useHostMetricsStore((s) => s.jetsonUpdatedAt);
  const live = isChappeLive();
  const metrics =
    live && liveMetrics && !hostMetricsStale(liveUpdatedAt)
      ? liveJetsonMetrics(liveMetrics) ?? dummyJetsonHostMetrics
      : (metricsProp ?? dummyJetsonHostMetrics);
  const debugLines =
    live && liveMetrics ? hostDebugLinesFromMetrics(liveMetrics) : [];

  return (
    <DashboardCardShell
      description="Jetson · perception"
      title={
        <span className="inline-flex items-center gap-1.5">
          {metrics.hostname}
          <HostDebugTooltip lines={debugLines} />
        </span>
      }
      action={
        <StatusBadge
          label={
            live && hostMetricsStale(liveUpdatedAt)
              ? 'stale'
              : metrics.online
                ? 'online'
                : 'offline'
          }
          tone={
            live && !hostMetricsStale(liveUpdatedAt) && metrics.online
              ? 'healthy'
              : 'muted'
          }
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
      footerSecondary={
        live && !hostMetricsStale(liveUpdatedAt)
          ? `${metrics.chappeRttMs.toFixed(1)} ms Chappe RTT`
          : live && hostMetricsStale(liveUpdatedAt)
            ? 'Host metrics stale'
            : undefined
      }
    />
  );
}
