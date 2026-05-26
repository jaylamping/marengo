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
import { useRobotStore } from '@/state/robotStore';

type PiHostCardProps = {
  metrics?: PiHostMetrics;
};

export function PiHostCard({ metrics = dummyPiHostMetrics }: PiHostCardProps) {
  const connected = useRobotStore((s) => s.connected);
  const operationalMode = useRobotStore((s) => s.operationalMode);
  const live = isChappeLive();

  async function handleEnableClick() {
    await postEnableCommand(true);
  }

  return (
    <DashboardCardShell
      description={live ? 'Pi 5 · Chappe gateway' : 'Pi 5 · onboard'}
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
          <StatusBadge
            label={
              live && connected && operationalMode
                ? operationalMode
                : metrics.throttled
                  ? 'throttled'
                  : 'healthy'
            }
            tone={
              live && connected
                ? 'healthy'
                : metrics.throttled
                  ? 'warning'
                  : 'healthy'
            }
          />
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
      footerSecondary={metrics.servicesLabel}
    />
  );
}
