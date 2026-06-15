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
import { hostCardDebugLines } from '@/lib/host-debug-info';
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
  const metricsLoading = live && liveMetrics === null;
  const metrics = live
    ? liveMetrics
      ? liveJetsonMetrics(liveMetrics)
      : null
    : (metricsProp ?? dummyJetsonHostMetrics);
  const stale = live && hostMetricsStale(liveUpdatedAt);
  const debugLines = hostCardDebugLines(live ? liveMetrics : null, {
    model: 'Jetson Orin Nano',
    subsystem: 'Fouché',
    host: metrics?.hostname,
    note: metricsLoading
      ? 'Waiting for telemetry'
      : stale
        ? 'Metrics stale'
        : live && metrics && !metrics.online
          ? 'Offline'
          : undefined,
  });
  const placeholder = '—';

  return (
    <DashboardCardShell
      title={
        <span className="inline-flex items-center gap-1.5">
          Jetson Orin Nano
          <HostDebugTooltip lines={debugLines} />
        </span>
      }
      action={
        <StatusBadge
          label={
            metricsLoading
              ? '…'
              : stale
                ? 'stale'
                : metrics?.online
                  ? 'online'
                  : 'offline'
          }
          tone={
            !metricsLoading && !stale && metrics?.online ? 'healthy' : 'muted'
          }
        />
      }
      content={
        <MetricGrid>
          <MetricItem
            label="CPU"
            value={placeholder}
            smoothValue={metrics?.cpuPercent}
            formatSmoothValue={formatPercent}
            usagePercent={metrics?.cpuPercent ?? 0}
          />
          <MetricItem
            label="RAM"
            value={
              metrics
                ? formatRamUsage(metrics.ramUsedGb, metrics.ramTotalGb)
                : placeholder
            }
            valueClassName="text-xs"
            usagePercent={
              metrics
                ? computeRamUsagePercent(metrics.ramUsedGb, metrics.ramTotalGb)
                : 0
            }
          />
          <MetricItem
            label="GPU"
            value={placeholder}
            smoothValue={metrics?.gpuPercent}
            formatSmoothValue={formatPercent}
            usagePercent={metrics?.gpuPercent ?? 0}
          />
          <MetricItem
            label="Temp"
            value={placeholder}
            smoothValue={metrics?.tempC}
            formatSmoothValue={formatTempC}
          />
        </MetricGrid>
      }
      footerPrimary={
        metricsLoading
          ? 'Waiting for telemetry'
          : metrics
            ? `Uptime ${metrics.uptime} · ${metrics.powerMode} · load ${formatLoad(metrics.load1m)}`
            : placeholder
      }
      footerSecondary={
        stale
          ? 'Host metrics stale'
          : metrics
            ? `${metrics.chappeRttMs.toFixed(1)} ms Chappe RTT`
            : undefined
      }
    />
  );
}
