import type { ReactNode } from 'react';
import { Area, AreaChart, CartesianGrid, XAxis } from 'recharts';

import {
  actuatorTrackingChartConfig,
  actuatorTrackingChartData,
} from '@/components/dashboard/inventory/constants';
import { SetLimitsPanel } from '@/components/dashboard/inventory/set-limits-panel';
import type { InventoryRow } from '@/components/dashboard/inventory/types';
import { Badge } from '@/components/ui/badge';
import { Button } from '@/components/ui/button';
import {
  ChartContainer,
  ChartTooltip,
  ChartTooltipContent,
} from '@/components/ui/chart';
import { Input } from '@/components/ui/input';
import { Label } from '@/components/ui/label';
import { Separator } from '@/components/ui/separator';
import { cn } from '@/lib/utils';

type ActuatorDetailBodyProps = {
  item: InventoryRow;
  interactive: boolean;
  limitDraft: string;
  onLimitDraftChange: (value: string) => void;
  onApplyRange: (range: string) => void;
  moveToDraft: string;
  onMoveToDraftChange: (value: string) => void;
};

/** Actuator-only command surface: telemetry, limits, actions, tests. */
export function ActuatorDetailBody({
  item,
  interactive,
  limitDraft,
  onLimitDraftChange,
  onApplyRange,
  moveToDraft,
  onMoveToDraftChange,
}: ActuatorDetailBodyProps) {
  return (
    <div className="flex flex-col gap-5">
      <InteractiveSection
        interactive={interactive}
        title="Telemetry"
        hint="Live readings unlock when this actuator is online and configured."
      >
        <div className="grid grid-cols-2 gap-2 sm:grid-cols-4">
          <MetricCell label="Pos" value={formatReading(item.value)} unit="rad" />
          <MetricCell label="Range" value={limitDraft || item.limit} />
          <MetricCell label="Bus V" value="—" unit="V" />
          <MetricCell label="Temp" value="—" unit="°C" />
          <MetricCell label="Torque" value="—" unit="Nm" />
          <MetricCell label="Velocity" value="—" unit="rad/s" />
          <MetricCell label="Fault" value={item.status === 'Fault' ? 'ACTIVE' : 'none'} />
          <MetricCell label="Node" value={item.node} />
        </div>
        <ChartContainer
          config={actuatorTrackingChartConfig}
          className="mt-3 aspect-[3/1] w-full"
        >
          <AreaChart
            accessibilityLayer
            data={actuatorTrackingChartData}
            margin={{ left: 0, right: 10, top: 4, bottom: 0 }}
          >
            <CartesianGrid vertical={false} />
            <XAxis
              dataKey="sample"
              tickLine={false}
              axisLine={false}
              tickMargin={8}
              hide
            />
            <ChartTooltip
              cursor={false}
              content={<ChartTooltipContent indicator="dot" />}
            />
            <Area
              dataKey="measured"
              type="natural"
              fill="var(--color-measured)"
              fillOpacity={0.6}
              stroke="var(--color-measured)"
              stackId="a"
            />
            <Area
              dataKey="commanded"
              type="natural"
              fill="var(--color-commanded)"
              fillOpacity={0.4}
              stroke="var(--color-commanded)"
              stackId="a"
            />
          </AreaChart>
        </ChartContainer>
        <p className="mt-2 text-xs text-muted-foreground">
          Tracking chart is a layout fixture until live CAN feedback and the
          shared time cursor land.
        </p>
      </InteractiveSection>

      <Separator className="bg-line" />

      <InteractiveSection
        interactive={interactive}
        title="Limits"
        hint="Set Limits and Set Zero require a live, configured actuator."
      >
        <SetLimitsPanel
          jointName={item.name}
          currentLimit={limitDraft}
          onApplyRange={(range) => {
            onLimitDraftChange(range);
            onApplyRange(range);
          }}
        />
      </InteractiveSection>

      <Separator className="bg-line" />

      <InteractiveSection
        interactive={interactive}
        title="Actions"
        hint="Homing and calibration stay locked until the link is interactive."
        brackets
      >
        <div className="flex flex-wrap gap-2">
          <Button type="button" size="sm" variant="panel" disabled={!interactive}>
            Homing
          </Button>
          <Button type="button" size="sm" variant="panel" disabled={!interactive}>
            Calibration
          </Button>
          <Button type="button" size="sm" variant="outline" disabled={!interactive}>
            Recover fault
          </Button>
        </div>
        <p className="text-xs text-muted-foreground">
          Homing / calibration post to gateway when those commands ship. Set
          Zero lives under Limits.
        </p>
      </InteractiveSection>

      <Separator className="bg-line" />

      <InteractiveSection
        interactive={interactive}
        title="Tests"
        hint="Motion tests stay locked on offline or unconfigured actuators."
        brackets
      >
        <div className="grid gap-3 sm:grid-cols-2">
          <div className="flex flex-col gap-2">
            <Label htmlFor="move-to" className="font-mono text-[10px] uppercase tracking-[0.14em]">
              Move to (rad)
            </Label>
            <div className="flex gap-2">
              <Input
                id="move-to"
                className="font-mono tabular-nums"
                value={moveToDraft}
                disabled={!interactive}
                onChange={(event) => onMoveToDraftChange(event.target.value)}
                placeholder="0.00"
              />
              <Button type="button" size="sm" disabled={!interactive}>
                Go
              </Button>
            </div>
          </div>
          <div className="flex flex-col gap-2">
            <Label className="font-mono text-[10px] uppercase tracking-[0.14em]">
              Sweep between limits
            </Label>
            <Button
              type="button"
              size="sm"
              variant="panel"
              disabled={!interactive}
              className="w-fit"
            >
              Start sweep
            </Button>
          </div>
        </div>
        <p className="text-xs text-muted-foreground">
          Move-to and sweep are UI-ready stubs — they do not queue motion until
          the harness command path is exposed here.
        </p>
      </InteractiveSection>
    </div>
  );
}

function InteractiveSection({
  interactive,
  title,
  hint,
  brackets,
  children,
}: {
  interactive: boolean;
  title: string;
  hint: string;
  brackets?: boolean;
  children: ReactNode;
}) {
  const header = (
    <div className="flex flex-wrap items-center justify-between gap-2">
      <h3 className="font-mono text-[10px] uppercase tracking-[0.14em] text-muted-foreground">
        {title}
      </h3>
      {!interactive ? (
        <Badge
          variant="outline"
          className="font-mono text-[10px] uppercase tracking-[0.14em]"
        >
          Locked
        </Badge>
      ) : null}
    </div>
  );

  const body = (
    <div
      className={cn(
        'flex flex-col gap-3',
        !interactive && 'pointer-events-none select-none',
      )}
      inert={!interactive ? true : undefined}
    >
      {children}
    </div>
  );

  return (
    <section
      className={cn('flex flex-col gap-3', !interactive && 'opacity-40')}
      aria-disabled={!interactive}
      data-interactive={interactive ? 'true' : 'false'}
    >
      {header}
      {/* Same shell as SetLimitsPanel: title sits above, content inset from brackets. */}
      {brackets ? (
        <div
          className={cn(
            'flex flex-col gap-3 rounded-sm border border-line p-3',
            interactive && 'panel-brackets',
          )}
        >
          {body}
        </div>
      ) : (
        body
      )}
      {!interactive ? (
        <p className="text-xs text-muted-foreground" role="status">
          {hint}
        </p>
      ) : null}
    </section>
  );
}

function MetricCell({
  label,
  value,
  unit,
}: {
  label: string;
  value: string;
  unit?: string;
}) {
  return (
    <div className="rounded-sm border border-line bg-surface-2 px-2 py-1.5">
      <div className="font-mono text-[10px] uppercase tracking-[0.14em] text-muted-foreground">
        {label}
      </div>
      <div className="font-mono tabular-nums text-foreground">
        {value}
        {unit && value !== '—' ? (
          <span className="ml-1 text-muted-foreground">{unit}</span>
        ) : null}
      </div>
    </div>
  );
}

function formatReading(value: string): string {
  if (value === '—' || value.trim() === '') {
    return '—';
  }
  return value;
}
