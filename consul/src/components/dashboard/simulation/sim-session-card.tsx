import { DashboardCardShell } from '@/components/dashboard/cards/dashboard-card-shell';
import {
  simDashboardCardShellClassName,
  simDataShellVariant,
} from '@/components/dashboard/simulation/constants';
import { MetricGrid } from '@/components/dashboard/metrics/metric-grid';
import { MetricItem } from '@/components/dashboard/metrics/metric-item';
import { StatusBadge } from '@/components/dashboard/metrics/status-badge';
import type { SimSession } from '@/data/simulation';

const sessionTone = {
  disconnected: 'muted',
  idle: 'warning',
  running: 'healthy',
  paused: 'warning',
} as const;

type SimSessionCardProps = {
  session: SimSession;
};

export function SimSessionCard({ session }: SimSessionCardProps) {
  return (
    <DashboardCardShell
      variant={simDataShellVariant}
      className={simDashboardCardShellClassName}
      description="Isaac Sim session"
      title={session.kitVersion}
      action={
        <StatusBadge label={session.state} tone={sessionTone[session.state]} />
      }
      content={
        <MetricGrid>
          <MetricItem label="Host" value={session.host} valueClassName="text-sm" />
          <MetricItem label="World" value={session.world} valueClassName="text-sm" />
          <MetricItem
            label="Robot USD"
            value={session.robotAsset}
            valueClassName="text-sm"
          />
          <MetricItem label="Extension" value={session.extension} valueClassName="text-sm" />
        </MetricGrid>
      }
      footerPrimary="Connect Kit on Jetson or workstation"
      footerSecondary="D2 tier · shares proto/ with Marengo stack"
    />
  );
}
