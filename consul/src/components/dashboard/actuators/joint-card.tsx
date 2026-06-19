import type { InventoryItem } from '@/data/robot-inventory';
import { TuningPanel } from '@/components/dashboard/actuators/tuning-panel';
import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from '@/components/ui/card';
import { dashboardPanelCardClassName } from '@/components/dashboard/layout/constants';
import { cn } from '@/lib/utils';

type JointCardProps = {
  joint: InventoryItem;
  wired: boolean;
};

function formatPosition(value: string): string {
  if (value === '—' || value.trim() === '') {
    return '—';
  }
  return `${value} rad`;
}

export function JointCard({ joint, wired }: JointCardProps) {
  const wiringLabel = wired ? 'Bench wired' : 'Not wired on bench';
  const modeLabel = wired ? 'Telemetry only until PR-5' : 'Telemetry only';

  return (
    <Card
      variant="panel"
      data-testid="joint-card"
      className={cn('@container/card', dashboardPanelCardClassName)}
    >
      <CardHeader>
        <CardDescription>{joint.node}</CardDescription>
        <CardTitle className="text-base font-semibold">{joint.name}</CardTitle>
      </CardHeader>
      <CardContent className="flex flex-col gap-2 px-6 pb-6 text-sm">
        <div className="flex items-baseline justify-between gap-2">
          <span className="text-muted-foreground">Position</span>
          <span className="font-mono tabular-nums">{formatPosition(joint.value)}</span>
        </div>
        <div className="flex items-baseline justify-between gap-2">
          <span className="text-muted-foreground">Limit envelope</span>
          <span className="font-mono tabular-nums">{joint.limit}</span>
        </div>
        <div className="flex items-baseline justify-between gap-2">
          <span className="text-muted-foreground">Status</span>
          <span>{joint.status}</span>
        </div>
        <p className="text-muted-foreground">{wiringLabel}</p>
        <p className="text-muted-foreground">{modeLabel}</p>
        <TuningPanel jointName={joint.name} wired={wired} />
      </CardContent>
    </Card>
  );
}
