import { DashboardCardShell } from '@/components/dashboard/cards/dashboard-card-shell';
import { MetricGrid } from '@/components/dashboard/metrics/metric-grid';
import { MetricItem } from '@/components/dashboard/metrics/metric-item';
import { StatusBadge } from '@/components/dashboard/metrics/status-badge';
import { Button } from '@/components/ui/button';
import {
  dummyPiHostMetrics,
  type PiHostMetrics,
} from '@/data/host-metrics';
import { postEnableCommand } from '@/lib/chappe-client';
import { isChappeLive } from '@/lib/chappe-config';
import {
  computeRamUsagePercent,
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

function livePiMetrics(
  metrics: ReturnType<typeof useHostMetricsStore.getState>['piMetrics'],
): PiHostMetrics | null {
  if (!metrics) {
    return null;
  }
  const cpu = metrics.cpu?.usagePercent ?? 0;
  const mem = metrics.memory;
  const ramTotalGb = mem ? Number(mem.totalBytes) / 1024 ** 3 : 0;
  const ramUsedGb = mem ? Number(mem.usedBytes) / 1024 ** 3 : 0;
  return {
    hostname: metrics.hostname || 'marengo-pi',
    cpuPercent: cpu,
    ramUsedGb,
    ramTotalGb,
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

  async function handleEnableClick() {
    await postEnableCommand(true);
  }

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
      action={
        <div className="flex items-center gap-2">
          {live && connected ? (
            <Button
              type="button"
              size="sm"
              variant="outline"
              className="h-7 text-xs"
              onClick={() => void handleEnableClick()}
            >
              Enable (HTTP)
            </Button>
          ) : null}
          <StatusBadge label={badgeLabel} tone={badgeTone} />
        </div>
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
          <MetricItem label="Temp" value={formatTempC(metrics.tempC)} />
          <MetricItem label="Load (1m)" value={formatLoad(metrics.load1m)} />
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
