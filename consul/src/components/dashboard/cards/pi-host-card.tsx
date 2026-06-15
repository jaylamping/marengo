import { DashboardCardShell } from '@/components/dashboard/cards/dashboard-card-shell';
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
import {
  canWarning,
  chappeDegraded,
  clockUnsynced,
  diskWarning,
  hostMetricsStale,
  useHostMetricsStore,
} from '@/state/hostMetricsStore';
import { useRobotStore } from '@/state/robotStore';

function formatUptime(seconds: bigint | number): string {
  const total = Number(seconds);
  const hours = Math.floor(total / 3600);
  const minutes = Math.floor((total % 3600) / 60);
  if (hours > 0) {
    return `${hours}h ${minutes}m`;
  }
  return `${minutes}m`;
}

function bytesToGb(bytes: bigint | number): number {
  return Number(bytes) / 1024 ** 3;
}

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
  const canIface = metrics.network?.find((iface) => iface.name.startsWith('can'));
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
    canState: canIface?.canState || (canIface?.up ? 'up' : null),
    tempC: metrics.thermal?.cpuCelsius ?? 0,
    load1m: metrics.load?.load1m ?? 0,
    uptime: formatUptime(metrics.uptimeSec),
    throttled:
      (metrics.platform.case === 'pi' && metrics.platform.value.throttledNow) ||
      (metrics.platform.case === 'pi' && metrics.platform.value.throttleEvents !== 0),
    servicesLabel: metrics.build?.deployRev
      ? `deploy ${metrics.build.deployRev}`
      : 'Chappe live',
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
  const metrics =
    live && liveMetrics && !hostMetricsStale(liveUpdatedAt)
      ? livePiMetrics(liveMetrics) ?? dummyPiHostMetrics
      : (metricsProp ?? dummyPiHostMetrics);

  const warnCan = live && canWarning(liveMetrics);
  const warnDisk = live && diskWarning(liveMetrics);
  const warnChappe = live && chappeDegraded(liveMetrics);
  const warnClock = live && clockUnsynced(liveMetrics);

  let badgeLabel = metrics.throttled ? 'throttled' : 'healthy';
  let badgeTone: 'healthy' | 'warning' | 'muted' = metrics.throttled
    ? 'warning'
    : 'healthy';
  if (live && connected && operationalMode) {
    badgeLabel = operationalMode;
    badgeTone = 'healthy';
  } else if (warnCan || warnDisk || warnChappe) {
    badgeLabel = warnCan ? 'CAN' : warnChappe ? 'chappe' : 'disk';
    badgeTone = 'warning';
  } else if (warnClock) {
    badgeLabel = 'clock';
    badgeTone = 'muted';
  }

  return (
    <DashboardCardShell
      description={
        live
          ? metrics.servicesLabel.startsWith('deploy')
            ? `Pi 5 · ${metrics.servicesLabel}`
            : 'Pi 5 · Chappe gateway'
          : 'Pi 5 · onboard'
      }
      title={metrics.hostname}
      action={<StatusBadge label={badgeLabel} tone={badgeTone} />}
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
          {metrics.diskUsedGb !== null && metrics.diskTotalGb !== null ? (
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
          {metrics.logDiskUsedGb !== null && metrics.logDiskBudgetGb !== null ? (
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
          <MetricItem label="Temp" value={formatTempC(metrics.tempC)} />
          <MetricItem label="Load (1m)" value={formatLoad(metrics.load1m)} />
          {metrics.canState ? (
            <MetricItem label="CAN" value={metrics.canState} valueClassName="text-xs" />
          ) : null}
        </MetricGrid>
      }
      footerPrimary={`Uptime ${metrics.uptime}`}
      footerSecondary={
        live && hostMetricsStale(liveUpdatedAt)
          ? 'Host metrics stale'
          : metrics.servicesLabel
      }
    />
  );
}
