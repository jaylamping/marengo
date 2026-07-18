import { DashboardCardShell } from '@/components/dashboard/cards/dashboard-card-shell';
import { HostDebugTooltip } from '@/components/dashboard/metrics/host-debug-tooltip';
import { MetricGrid } from '@/components/dashboard/metrics/metric-grid';
import { MetricItem } from '@/components/dashboard/metrics/metric-item';
import { StatusBadge } from '@/components/dashboard/metrics/status-badge';
import {
  dummyPiHostMetrics,
  type PiHostMetrics,
} from '@/data/host-metrics';
import { isChappeLive } from '@/lib/chappe-config';
import {
  computeRamUsagePercent,
  computeUsagePercent,
  formatLoad,
  formatPercent,
  formatRamUsage,
  formatTempC,
} from '@/lib/format';
import { hostCardDebugLines } from '@/lib/host-debug-info';
import {
  canWarning,
  chappeDegraded,
  clockUnsynced,
  diskWarning,
  hostMetricsStale,
  useHostMetricsStore,
} from '@/state/hostMetricsStore';
import { useRobotStore } from '@/state/robotStore';
import { formatUptime, bytesToGb } from '@/lib/host-card-utils';


function livePiMetrics(
  metrics: ReturnType<typeof useHostMetricsStore.getState>['piMetrics'],
): PiHostMetrics | null {
  if (!metrics) {
    return null;
  }
  const cpu = metrics.cpu?.usagePercent ?? 0;
  const mem = metrics.memory;
  const ramTotalGb = mem ? bytesToGb(mem.totalBytes) : 0;
  const ramUsedGb = mem ? bytesToGb(mem.usedBytes) : 0;
  const rootDisk =
    metrics.disks?.find((disk) => disk.mountPoint === '/') ?? metrics.disks?.[0];
  const logBudget = metrics.logDiskBudgetBytes;
  return {
    hostname: metrics.hostname || 'marengo-pi',
    cpuPercent: cpu,
    ramUsedGb,
    ramTotalGb,
    diskUsedGb: rootDisk ? bytesToGb(rootDisk.usedBytes) : null,
    diskTotalGb: rootDisk ? bytesToGb(rootDisk.totalBytes) : null,
    logDiskUsedGb:
      metrics.logDiskBytes !== undefined ? bytesToGb(metrics.logDiskBytes) : null,
    logDiskBudgetGb:
      logBudget !== undefined && logBudget > 0n ? bytesToGb(logBudget) : null,
    tempC: metrics.thermal?.cpuCelsius ?? 0,
    load1m: metrics.load?.load1m ?? 0,
    uptime: formatUptime(metrics.uptimeSec),
    throttled:
      (metrics.platform.case === 'pi' && metrics.platform.value.throttledNow) ||
      (metrics.platform.case === 'pi' && metrics.platform.value.throttleEvents !== 0),
  };
}

type PiHostCardProps = {
  metrics?: PiHostMetrics;
};

export function PiHostCard({ metrics: metricsProp }: PiHostCardProps) {
  const connected = useRobotStore((s) => s.connected);
  const operationalMode = useRobotStore((s) => s.operationalMode);
  const liveMetrics = useHostMetricsStore((s) => s.piMetrics);
  const liveUpdatedAt = useHostMetricsStore((s) => s.piUpdatedAt);
  const live = isChappeLive();
  const metricsLoading = live && liveMetrics === null;
  const metrics = live
    ? liveMetrics
      ? livePiMetrics(liveMetrics)
      : null
    : (metricsProp ?? dummyPiHostMetrics);
  const stale = live && hostMetricsStale(liveUpdatedAt);
  const placeholder = '—';

  const warnCan = live && canWarning(liveMetrics);
  const warnDisk = live && diskWarning(liveMetrics);
  const warnChappe = live && chappeDegraded(liveMetrics);
  const warnClock = live && clockUnsynced(liveMetrics);
  const debugLines = hostCardDebugLines(live ? liveMetrics : null, {
    model: 'PI 5',
    subsystem: 'Chappe',
    host: metrics?.hostname,
    note: metricsLoading ? 'Waiting for telemetry' : stale ? 'Metrics stale' : undefined,
  });

  let badgeLabel = metricsLoading ? '…' : metrics?.throttled ? 'throttled' : 'healthy';
  let badgeTone: 'healthy' | 'warning' | 'muted' = metrics?.throttled
    ? 'warning'
    : 'healthy';
  if (metricsLoading) {
    badgeTone = 'muted';
  } else if (warnCan || warnDisk || warnChappe) {
    badgeLabel = warnCan ? 'CAN' : warnChappe ? 'chappe' : 'disk';
    badgeTone = 'warning';
  } else if (warnClock) {
    badgeLabel = 'clock';
    badgeTone = 'muted';
  } else if (live && connected && operationalMode) {
    badgeLabel = operationalMode;
    badgeTone = 'healthy';
  }

  return (
    <DashboardCardShell
      title={
        <span className="inline-flex items-center gap-1.5">
          PI 5
          <HostDebugTooltip lines={debugLines} />
        </span>
      }
      action={<StatusBadge label={badgeLabel} tone={badgeTone} />}
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
          {metrics?.diskUsedGb != null && metrics.diskTotalGb != null ? (
            <MetricItem
              label="Disk"
              value={formatRamUsage(metrics.diskUsedGb, metrics.diskTotalGb)}
              valueClassName="text-xs"
              usagePercent={computeUsagePercent(
                metrics.diskUsedGb,
                metrics.diskTotalGb,
              )}
            />
          ) : null}
          {metrics?.logDiskUsedGb != null && metrics.logDiskBudgetGb != null ? (
            <MetricItem
              label="Logs"
              value={formatRamUsage(
                metrics.logDiskUsedGb,
                metrics.logDiskBudgetGb,
              )}
              valueClassName="text-xs"
              usagePercent={computeUsagePercent(
                metrics.logDiskUsedGb,
                metrics.logDiskBudgetGb,
              )}
            />
          ) : null}
          <MetricItem
            label="Temp"
            value={placeholder}
            smoothValue={metrics?.tempC}
            formatSmoothValue={formatTempC}
          />
          <MetricItem
            label="Load (1m)"
            value={placeholder}
            smoothValue={metrics?.load1m}
            formatSmoothValue={formatLoad}
          />
        </MetricGrid>
      }
      footerPrimary={
        metricsLoading
          ? 'Waiting for telemetry'
          : metrics
            ? `Uptime ${metrics.uptime}`
            : placeholder
      }
      footerSecondary={
        stale ? 'Host metrics stale' : undefined
      }
    />
  );
}
