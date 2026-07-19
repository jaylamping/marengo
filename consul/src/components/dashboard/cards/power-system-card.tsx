import {
  BatteryPackLegend,
  BatterySocRings,
} from '@/components/dashboard/cards/battery-soc-rings';
import { DashboardCardShell } from '@/components/dashboard/cards/dashboard-card-shell';
import { MetricGrid } from '@/components/dashboard/metrics/metric-grid';
import { MetricItem } from '@/components/dashboard/metrics/metric-item';
import { StatusBadge } from '@/components/dashboard/metrics/status-badge';
import {
  computeAggregateSoc,
  getSystemStatus,
} from '@/data/battery-metrics';
import {
  dummyPowerSystemMetrics,
  type PowerSystemMetrics,
} from '@/data/power-system';
import { isChappeLive } from '@/lib/chappe-config';
import {
  formatCurrentA,
  formatEnergyWh,
  formatPowerW,
  formatRuntimeMin,
  formatTempC,
  formatVoltageV,
} from '@/lib/format';
import { demoBadge } from '@/lib/telemetry-source';

type PowerSystemCardProps = {
  metrics?: PowerSystemMetrics;
};

const systemStatusTone = {
  discharging: 'healthy',
  charging: 'healthy',
  idle: 'muted',
  offline: 'warning',
  mixed: 'warning',
} as const;

export function PowerSystemCard({ metrics: metricsProp }: PowerSystemCardProps) {
  const live = isChappeLive();
  // No live power feed yet — never present dummy as healthy machine truth.
  const usingFixture = metricsProp === undefined;
  const metrics = metricsProp ?? dummyPowerSystemMetrics;
  const { board, batteries } = metrics;
  const aggregateSocPercent = computeAggregateSoc(batteries.packs);
  const systemStatus = getSystemStatus(batteries.packs);
  const honesty = !live
    ? demoBadge()
    : usingFixture
      ? demoBadge('no feed')
      : null;

  return (
    <DashboardCardShell
      description="Power board · batteries"
      title={`${aggregateSocPercent}% · ${formatPowerW(board.powerW)}`}
      action={
        <div className="flex flex-wrap justify-end gap-2">
          {honesty ? (
            <StatusBadge label={honesty.label} tone={honesty.tone} />
          ) : (
            <>
              <StatusBadge
                label={systemStatus}
                tone={systemStatusTone[systemStatus]}
              />
              <StatusBadge
                label={board.healthy ? 'board ok' : 'board fault'}
                tone={board.healthy ? 'healthy' : 'warning'}
              />
            </>
          )}
        </div>
      }
      content={
        <div className="grid gap-5 @md/card:grid-cols-2">
          <div className="space-y-3">
            <p className="text-xs font-medium uppercase tracking-wide text-muted-foreground">
              Shunt · {board.boardName}
            </p>
            <MetricGrid>
              <MetricItem label="Bus" value={formatVoltageV(board.busVoltageV)} />
              <MetricItem label="Current" value={formatCurrentA(board.shuntCurrentA)} />
              <MetricItem label="Power" value={formatPowerW(board.powerW)} />
              <MetricItem label="Energy" value={formatEnergyWh(board.energyWhSession)} />
            </MetricGrid>
          </div>

          <div className="space-y-3">
            <p className="text-xs font-medium uppercase tracking-wide text-muted-foreground">
              Packs · {batteries.packs.length} online
            </p>
            <BatterySocRings
              packs={batteries.packs}
              aggregateSocPercent={aggregateSocPercent}
              className="max-w-[180px] @md/card:mx-0"
            />
            <BatteryPackLegend packs={batteries.packs} />
          </div>
        </div>
      }
      footerPrimary={
        honesty
          ? live
            ? 'Power feed not connected — layout fixture'
            : 'Wireframe · synthetic power'
          : `${formatRuntimeMin(batteries.estimatedRuntimeMin)} remaining · Peak ${formatPowerW(board.peakPowerW)} · ${formatTempC(board.boardTempC)} · ${board.shuntRatingA} A shunt`
      }
      footerSecondary={
        honesty
          ? 'Not measured from Chappe'
          : `${board.detail} · ${batteries.detail}`
      }
    />
  );
}
