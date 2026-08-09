import { useMemo, type ReactNode } from 'react';
import { CartesianGrid, Line, LineChart, XAxis, YAxis } from 'recharts';

import { actuatorTelemetryChartConfig } from '@/components/dashboard/inventory/constants';
import { HomeActuatorButton } from '@/components/dashboard/inventory/home-actuator-button';
import { InventoryLimitsReadOnly } from '@/components/dashboard/hardware/hardware-settings-sheet';
import type { InventoryRow } from '@/components/dashboard/inventory/types';
import type { JointTrackingPoint } from '@/components/dashboard/charts/types';
import { Badge } from '@/components/ui/badge';
import { Button } from '@/components/ui/button';
import {
  ChartContainer,
  ChartLegend,
  ChartLegendContent,
  ChartTooltip,
  ChartTooltipContent,
} from '@/components/ui/chart';
import { Input } from '@/components/ui/input';
import { Label } from '@/components/ui/label';
import { Separator } from '@/components/ui/separator';
import { cn } from '@/lib/utils';
import { useRobotStore } from '@/state/robotStore';

type ActuatorDetailBodyProps = {
  item: InventoryRow;
  interactive: boolean;
  limitDraft: string;
  onLimitDraftChange: (value: string) => void;
  onApplyRange: (range: string) => void;
  moveToDraft: string;
  onMoveToDraftChange: (value: string) => void;
};

type TelemetrySeriesKey = 'position' | 'torque' | 'velocity' | 'temperature';

type ChartRow = {
  time: string;
  position: number;
  torque: number;
  velocity: number;
  temperature: number;
  positionRaw: number;
  torqueRaw: number;
  velocityRaw: number;
  temperatureRaw: number;
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
  const connected = useRobotStore((s) => s.connected);
  const joint = useRobotStore((s) =>
    s.robotState?.joints.find((entry) => entry.name === item.name),
  );
  const history = useRobotStore(
    (s) => s.trackingPointsByJoint[item.name] ?? EMPTY_POINTS,
  );

  const live = connected && joint !== undefined;
  const posValue = live
    ? formatFixed(joint.position, 3)
    : formatReading(item.value);
  const velocityValue = live ? formatFixed(joint.velocity, 3) : '—';
  const torqueValue = live ? formatFixed(joint.effort, 3) : '—';
  const tempValue = live ? formatFixed(joint.temperatureC, 1) : '—';
  const faultValue = live
    ? joint.fault === 0
      ? 'none'
      : `0x${joint.fault.toString(16).padStart(4, '0')}`
    : item.status === 'Fault'
      ? 'ACTIVE'
      : 'none';

  const chartData = useMemo(() => buildNormalizedChartRows(history), [history]);

  return (
    <div className="flex flex-col gap-5">
      <InteractiveSection
        interactive={interactive}
        title="Telemetry"
        hint="Live readings unlock when this actuator is online and configured."
      >
        <ChartContainer
          config={actuatorTelemetryChartConfig}
          className="aspect-[3/1] w-full"
        >
          <LineChart
            accessibilityLayer
            data={chartData}
            margin={{ left: 0, right: 10, top: 4, bottom: 0 }}
          >
            <CartesianGrid vertical={false} />
            <XAxis
              dataKey="time"
              tickLine={false}
              axisLine={false}
              tickMargin={8}
              hide
            />
            <YAxis hide domain={[0, 1]} />
            <ChartTooltip
              cursor={false}
              content={
                <ChartTooltipContent
                  indicator="line"
                  formatter={(value, name, item) => {
                    const key = String(name) as TelemetrySeriesKey;
                    const row = item.payload as ChartRow | undefined;
                    const raw = rawFromRow(row, key);
                    const label =
                      actuatorTelemetryChartConfig[key]?.label ?? key;
                    return (
                      <div className="flex w-full items-center justify-between gap-4">
                        <span className="text-muted-foreground">{label}</span>
                        <span className="font-mono tabular-nums text-foreground">
                          {raw === undefined
                            ? formatFixed(Number(value), 3)
                            : formatTooltip(key, raw)}
                        </span>
                      </div>
                    );
                  }}
                />
              }
            />
            <ChartLegend content={<ChartLegendContent />} />
            <Line
              dataKey="position"
              type="monotone"
              stroke="var(--color-position)"
              strokeWidth={1.5}
              dot={false}
              isAnimationActive={false}
            />
            <Line
              dataKey="torque"
              type="monotone"
              stroke="var(--color-torque)"
              strokeWidth={1.5}
              dot={false}
              isAnimationActive={false}
            />
            <Line
              dataKey="velocity"
              type="monotone"
              stroke="var(--color-velocity)"
              strokeWidth={1.5}
              dot={false}
              isAnimationActive={false}
            />
            <Line
              dataKey="temperature"
              type="monotone"
              stroke="var(--color-temperature)"
              strokeWidth={1.5}
              dot={false}
              isAnimationActive={false}
            />
          </LineChart>
        </ChartContainer>
        <div className="grid grid-cols-2 gap-2 sm:grid-cols-4">
          <MetricCell label="Pos" value={posValue} unit="rad" />
          <MetricCell label="Range" value={limitDraft || item.limit} />
          <MetricCell label="Bus V" value="—" unit="V" />
          <MetricCell label="Temp" value={tempValue} unit="°C" />
          <MetricCell label="Torque" value={torqueValue} unit="Nm" />
          <MetricCell label="Velocity" value={velocityValue} unit="rad/s" />
          <MetricCell label="Fault" value={faultValue} />
          <MetricCell label="Node" value={item.node} />
        </div>
        <p className="text-xs text-muted-foreground">
          Series are normalized to share the plot; tooltip shows engineering
          units. Bus V is not in MIT / type-24 feedback yet.
        </p>
      </InteractiveSection>

      <Separator className="bg-line" />

      <InteractiveSection
        interactive={interactive}
        title="Limits"
        hint="Set Limits moved to Hardware — live range shown read-only here."
      >
        <InventoryLimitsReadOnly
          jointName={item.name}
          liveRange={limitDraft || item.limit}
        />
      </InteractiveSection>

      <Separator className="bg-line" />

      <InteractiveSection
        interactive={interactive}
        title="Actions"
        hint="Home moves a zero'd joint to 0 rad. Calibration stays locked until that path ships."
        brackets
      >
        <div className="flex flex-wrap items-start gap-2">
          <HomeActuatorButton
            jointName={item.name}
            interactive={interactive}
          />
          <Button type="button" size="sm" variant="panel" disabled={!interactive}>
            Calibration
          </Button>
          <Button type="button" size="sm" variant="outline" disabled={!interactive}>
            Recover fault
          </Button>
        </div>
        <p className="text-xs text-muted-foreground">
          Home requires Set Zero (Limits) then Enable / ACTIVE. Calibration
          posts to gateway when that command ships.
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

const EMPTY_POINTS: JointTrackingPoint[] = [];

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

function formatFixed(value: number, digits: number): string {
  if (!Number.isFinite(value)) {
    return '—';
  }
  return value.toFixed(digits);
}

function formatTooltip(key: TelemetrySeriesKey, raw: number): string {
  switch (key) {
    case 'position':
      return `${formatFixed(raw, 3)} rad`;
    case 'torque':
      return `${formatFixed(raw, 3)} Nm`;
    case 'velocity':
      return `${formatFixed(raw, 3)} rad/s`;
    case 'temperature':
      return `${formatFixed(raw, 1)} °C`;
    default: {
      const _exhaustive: never = key;
      return _exhaustive;
    }
  }
}

function rawFromRow(
  row: ChartRow | undefined,
  key: TelemetrySeriesKey,
): number | undefined {
  if (!row) {
    return undefined;
  }
  switch (key) {
    case 'position':
      return row.positionRaw;
    case 'torque':
      return row.torqueRaw;
    case 'velocity':
      return row.velocityRaw;
    case 'temperature':
      return row.temperatureRaw;
    default: {
      const _exhaustive: never = key;
      return _exhaustive;
    }
  }
}

function buildNormalizedChartRows(history: JointTrackingPoint[]): ChartRow[] {
  if (history.length === 0) {
    return [];
  }
  const series = history.map((point) => ({
    time: point.time,
    position: point.measured,
    torque: point.torque ?? 0,
    velocity: point.velocity ?? 0,
    temperature: point.temperature ?? 0,
  }));
  const positionScale = seriesScale(series.map((p) => p.position));
  const torqueScale = seriesScale(series.map((p) => p.torque));
  const velocityScale = seriesScale(series.map((p) => p.velocity));
  const temperatureScale = seriesScale(series.map((p) => p.temperature));

  return series.map((point) => ({
    time: point.time,
    position: normalize(point.position, positionScale),
    torque: normalize(point.torque, torqueScale),
    velocity: normalize(point.velocity, velocityScale),
    temperature: normalize(point.temperature, temperatureScale),
    positionRaw: point.position,
    torqueRaw: point.torque,
    velocityRaw: point.velocity,
    temperatureRaw: point.temperature,
  }));
}

function seriesScale(values: number[]): { min: number; max: number } {
  let min = Number.POSITIVE_INFINITY;
  let max = Number.NEGATIVE_INFINITY;
  for (const value of values) {
    if (!Number.isFinite(value)) {
      continue;
    }
    min = Math.min(min, value);
    max = Math.max(max, value);
  }
  if (!Number.isFinite(min) || !Number.isFinite(max)) {
    return { min: 0, max: 1 };
  }
  if (Math.abs(max - min) < 1e-6) {
    return { min: min - 0.5, max: max + 0.5 };
  }
  return { min, max };
}

function normalize(value: number, scale: { min: number; max: number }): number {
  return (value - scale.min) / (scale.max - scale.min);
}
