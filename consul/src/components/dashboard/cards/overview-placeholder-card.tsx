import { DashboardCardShell } from '@/components/dashboard/cards/dashboard-card-shell';
import { StatusBadge } from '@/components/dashboard/metrics/status-badge';

export function OverviewPlaceholderCard() {
  return (
    <DashboardCardShell
      description="Overview slot"
      title="TBD"
      action={<StatusBadge label="placeholder" tone="muted" />}
      content={
        <div className="flex min-h-[132px] items-center justify-center rounded-lg border border-dashed bg-muted/10 px-4 text-center text-sm text-muted-foreground">
          Fourth KPI card reserved — swap in when you know what belongs here.
        </div>
      }
      footerPrimary="Keeps the 4-column overview grid"
      footerSecondary="Control loop, CAN, safety, or something else"
    />
  );
}
