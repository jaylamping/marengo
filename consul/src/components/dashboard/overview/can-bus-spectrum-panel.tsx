import { Link } from 'react-router-dom';
import {
  CartesianGrid,
  Line,
  LineChart,
  ReferenceLine,
  XAxis,
  YAxis,
} from 'recharts';

import { dashboardPanelCardClassName } from '@/components/dashboard/layout/constants';
import {
  Card,
  CardAction,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from '@/components/ui/card';
import {
  ChartContainer,
  ChartTooltip,
  ChartTooltipContent,
  type ChartConfig,
} from '@/components/ui/chart';
import { useCanTrafficSpectrum } from '@/hooks/use-can-traffic-spectrum';
import {
  LOGS_CAN_HREF,
  type CanIdBand,
  type CanLinkActivitySample,
  type CanLiveChip,
  type CaptureState,
  type HzSample,
  type InterfacePartition,
  type MicroLogLine,
  type SpectrumDump,
} from '@/lib/can-traffic-spectrum';
import { logErrorMessage, shouldShowLogErrorBanner } from '@/lib/log-api';
import { cn } from '@/lib/utils';

const linkActivityChartConfig = {
  rx: {
    label: 'RX',
    color: 'var(--info)',
  },
  tx: {
    label: 'TX',
    color: 'var(--chart-4)',
  },
} satisfies ChartConfig;

type CanBusSpectrumPanelProps = {
  active?: boolean;
};

function presenceCopy(capture: CaptureState): string {
  switch (capture.status) {
    case 'unavailable':
      return 'Capture unavailable';
    case 'empty':
      return 'No harness capture';
    case 'ready':
      return capture.dump.freshness === 'stale' ? 'Dump unchanged' : 'Dump updating';
    default: {
      const _exhaustive: never = capture;
      return _exhaustive;
    }
  }
}

function formatHz(value: number | null): string {
  if (value == null || !Number.isFinite(value)) {
    return '—';
  }
  return value.toFixed(1);
}

function shortJoint(name: string | undefined): string {
  if (!name) {
    return '—';
  }
  return name
    .replace(/^right_/, 'r_')
    .replace(/^left_/, 'l_')
    .replace(/shoulder_/, 'sh_')
    .replace(/upper_arm_/, 'ua_')
    .replace(/lower_arm_/, 'la_');
}

function LiveChip({ live }: { live: CanLiveChip }) {
  const label = live.iface ?? 'can';
  const state = live.canState?.length ? live.canState : '—';
  const known = state !== '—';
  const nominal = !live.warn && state === 'ERROR-ACTIVE';
  return (
    <span
      className={cn(
        'inline-flex items-center gap-1.5 font-mono text-[10px] uppercase tracking-[0.14em]',
        live.warn ? 'text-warning' : 'text-muted-foreground',
      )}
      data-testid="can-live-chip"
      title={
        live.txErrorCount != null || live.rxErrorCount != null
          ? `err tx ${live.txErrorCount ?? '—'} / rx ${live.rxErrorCount ?? '—'}`
          : undefined
      }
    >
      <span
        className={cn(
          'led',
          live.warn && 'led-accent',
          !live.warn && known && 'led-ok',
          nominal && 'led-live',
        )}
        aria-hidden
      />
      {label}
      <span className="text-foreground/80">·</span>
      {state}
    </span>
  );
}

function MetricReadout({
  label,
  value,
  unit,
}: {
  label: string;
  value: string;
  unit?: string;
}) {
  return (
    <div className="min-w-0">
      <div className="micro-label">{label}</div>
      <div className="mt-0.5 flex items-baseline gap-1 font-mono tabular-nums">
        <span className="data-value text-sm text-foreground">{value}</span>
        {unit ? (
          <span className="text-[10px] uppercase tracking-[0.14em] text-muted-foreground">
            {unit}
          </span>
        ) : null}
      </div>
    </div>
  );
}

function IdHistogram({ bands }: { bands: CanIdBand[] }) {
  if (bands.length === 0) {
    return (
      <p className="font-mono text-[11px] text-muted-foreground">No ID traffic</p>
    );
  }
  const maxShare = Math.max(...bands.map((b) => b.share), 0.001);
  return (
    <div className="flex flex-col gap-1" data-testid="can-id-histogram">
      {bands.slice(0, 5).map((band) => (
        <div
          key={band.canId}
          className="grid grid-cols-[4.75rem_minmax(0,1fr)_2.25rem] items-center gap-2"
        >
          <span className="truncate font-mono text-[11px] tabular-nums text-foreground">
            {band.canId}
          </span>
          <div className="h-1 overflow-hidden rounded-[2px] bg-surface-2">
            <div
              className="h-full bg-info/90 transition-[width] duration-150 ease-out motion-reduce:transition-none"
              style={{ width: `${Math.max(3, (band.share / maxShare) * 100)}%` }}
            />
          </div>
          <span className="text-right font-mono text-[10px] tabular-nums text-muted-foreground">
            {band.count}
          </span>
        </div>
      ))}
    </div>
  );
}

function RateSparkline({ samples }: { samples: HzSample[] }) {
  if (samples.length < 2) {
    return (
      <div
        className="flex h-7 items-end font-mono text-[10px] uppercase tracking-[0.14em] text-muted-foreground"
        data-testid="can-rate-sparkline"
      >
        rate —
      </div>
    );
  }
  const max = Math.max(...samples.map((s) => s.hz), 1);
  const width = 160;
  const height = 28;
  const points = samples
    .map((sample, index) => {
      const x = (index / (samples.length - 1)) * width;
      const y = height - (sample.hz / max) * (height - 4) - 2;
      return `${x},${y}`;
    })
    .join(' ');
  return (
    <svg
      viewBox={`0 0 ${width} ${height}`}
      className="h-7 w-full text-info"
      data-testid="can-rate-sparkline"
      aria-hidden
    >
      <polyline
        fill="none"
        stroke="currentColor"
        strokeWidth="1.25"
        strokeLinejoin="round"
        strokeLinecap="round"
        points={points}
      />
    </svg>
  );
}

function InterfaceStrip({ partitions }: { partitions: InterfacePartition[] }) {
  if (partitions.length === 0) {
    return null;
  }
  return (
    <div className="flex flex-wrap gap-x-3 gap-y-1">
      {partitions.map((part) => (
        <span
          key={part.name}
          className="font-mono text-[10px] uppercase tracking-[0.14em] text-muted-foreground"
        >
          <span className="text-foreground/90">{part.name}</span>
          {' · '}
          {part.frameCount}
          {part.approxHz != null ? ` · ${part.approxHz.toFixed(0)} Hz` : ''}
        </span>
      ))}
    </div>
  );
}

function MicroLog({ lines }: { lines: MicroLogLine[] }) {
  if (lines.length === 0) {
    return null;
  }
  return (
    <div
      className="flex min-w-0 shrink-0 flex-col overflow-hidden rounded-[4px] border border-line bg-surface-0"
      data-testid="can-micro-log"
    >
      <div className="grid grid-cols-[3.5rem_2rem_4.5rem_minmax(0,1fr)_minmax(5rem,1.2fr)] gap-1 border-b border-line px-2 py-1">
        {(['Δt', 'if', 'id', 'joint', 'data'] as const).map((label) => (
          <span key={label} className="micro-label">
            {label}
          </span>
        ))}
      </div>
      <ul className="h-[8.5rem] divide-y divide-line overflow-y-auto">
        {[...lines].reverse().map((line) => (
          <li
            key={`${line.lineNo}-${line.canId}-${line.offsetS}`}
            className="grid grid-cols-[3.5rem_2rem_4.5rem_minmax(0,1fr)_minmax(5rem,1.2fr)] gap-1 px-2 py-[3px] font-mono text-[11px] tabular-nums text-foreground"
          >
            <span className="text-muted-foreground">{line.offsetS.toFixed(3)}</span>
            <span>{line.iface.replace(/^can/, '')}</span>
            <span>{line.canId}</span>
            <span className="truncate text-muted-foreground" title={line.joint}>
              {shortJoint(line.joint) !== '—'
                ? shortJoint(line.joint)
                : (line.commTypeName ?? '—')}
            </span>
            <span className="truncate text-muted-foreground" title={line.dataHead}>
              {line.dataHead}
            </span>
          </li>
        ))}
      </ul>
    </div>
  );
}

function formatBps(value: number): string {
  if (!Number.isFinite(value) || value <= 0) {
    return '0';
  }
  if (value >= 1000) {
    return `${(value / 1000).toFixed(1)}k`;
  }
  return Math.round(value).toString();
}

function CaptureStatus({
  title,
  detail,
}: {
  title: string;
  detail: string;
}) {
  return (
    <div
      className="shrink-0 rounded-[4px] border border-line bg-surface-0 px-3 py-2.5"
      data-testid="can-capture-status"
    >
      <p className="font-mono text-[11px] uppercase tracking-[0.14em] text-foreground">
        {title}
      </p>
      <p className="mt-0.5 max-w-[42ch] text-xs text-muted-foreground">{detail}</p>
    </div>
  );
}

function SeriesKey({
  color,
  label,
}: {
  color: string;
  label: string;
}) {
  return (
    <span className="inline-flex items-center gap-1 font-mono text-[10px] uppercase tracking-[0.14em] text-muted-foreground">
      <span
        className="size-1.5 shrink-0 rounded-[1px]"
        style={{ background: color }}
        aria-hidden
      />
      {label}
    </span>
  );
}

function LinkActivityChart({ samples }: { samples: CanLinkActivitySample[] }) {
  const data = samples.map((sample) => ({
    t: sample.atMs,
    rx: sample.rxBps,
    tx: sample.txBps,
  }));
  const latest = samples[samples.length - 1];

  return (
    <div
      className="flex shrink-0 flex-col gap-1 border-t border-line pt-2.5"
      data-testid="can-link-activity"
    >
      <div className="flex flex-wrap items-baseline justify-between gap-x-3 gap-y-1">
        <div className="flex items-center gap-3">
          <div className="micro-label">Link</div>
          <SeriesKey color="var(--info)" label="RX" />
          <SeriesKey color="var(--chart-4)" label="TX" />
        </div>
        <span className="font-mono text-[10px] tabular-nums text-muted-foreground">
          <span className="text-foreground/90">{formatBps(latest?.rxBps ?? 0)}</span>
          {' / '}
          <span className="text-foreground/90">{formatBps(latest?.txBps ?? 0)}</span>
          {' B/s'}
        </span>
      </div>
      <ChartContainer
        config={linkActivityChartConfig}
        className="aspect-auto h-[5rem] w-full min-h-[5rem]"
        initialDimension={{ width: 960, height: 80 }}
      >
        <LineChart
          accessibilityLayer
          data={data}
          margin={{ left: 0, right: 4, top: 6, bottom: 2 }}
        >
          <CartesianGrid
            vertical={false}
            stroke="var(--line)"
            strokeDasharray="2 4"
          />
          <XAxis dataKey="t" hide />
          <YAxis
            width={28}
            tickLine={false}
            axisLine={false}
            tickMargin={4}
            tick={{ fontSize: 10, fontFamily: 'var(--font-mono)' }}
            tickFormatter={formatBps}
            domain={[0, (max: number) => Math.max(8, max * 1.15)]}
          />
          {/* Zero rail — idle bus still reads as a live instrument. */}
          <ReferenceLine y={0} stroke="var(--line-strong)" strokeWidth={1} />
          <ChartTooltip
            cursor={false}
            isAnimationActive={false}
            content={
              <ChartTooltipContent
                labelFormatter={() => 'B/s'}
                indicator="line"
              />
            }
          />
          <Line
            dataKey="rx"
            type="monotone"
            stroke="var(--color-rx)"
            strokeWidth={1.5}
            dot={false}
            isAnimationActive={false}
          />
          <Line
            dataKey="tx"
            type="monotone"
            stroke="var(--color-tx)"
            strokeWidth={1.5}
            dot={false}
            isAnimationActive={false}
          />
        </LineChart>
      </ChartContainer>
    </div>
  );
}

function HotDumpBody({ dump }: { dump: SpectrumDump }) {
  const tailError =
    dump.tailError != null && shouldShowLogErrorBanner(dump.tailError)
      ? dump.tailError
      : null;
  return (
    <div className="grid gap-3 @3xl/card:grid-cols-[10.5rem_minmax(14rem,0.85fr)_minmax(0,1.35fr)]">
      <div className="flex min-w-0 flex-col gap-2.5 border-b border-line pb-2.5 @3xl/card:border-b-0 @3xl/card:border-r @3xl/card:pb-0 @3xl/card:pr-3">
        <div className="grid grid-cols-3 gap-3 @3xl/card:grid-cols-1 @3xl/card:gap-2">
          <MetricReadout
            label="Rate"
            value={formatHz(dump.sessionApproxHz)}
            unit="Hz"
          />
          <MetricReadout label="Frames" value={dump.parsedFrames.toLocaleString()} />
          <MetricReadout
            label="Window"
            value={dump.durationS.toFixed(2)}
            unit="s"
          />
        </div>
        <InterfaceStrip partitions={dump.partitions} />
      </div>
      <div className="flex min-w-0 flex-col gap-2.5 border-b border-line pb-2.5 @3xl/card:border-b-0 @3xl/card:border-r @3xl/card:pb-0 @3xl/card:pr-3">
        <div className="min-w-0 space-y-1">
          <div className="micro-label">Top IDs</div>
          <IdHistogram bands={dump.bands} />
        </div>
        <div className="min-w-0 space-y-1">
          <div className="micro-label">Bus rate</div>
          <RateSparkline samples={dump.rateHz} />
        </div>
      </div>
      <div className="flex min-w-0 flex-col gap-1">
        <div className="flex items-baseline justify-between gap-2">
          <div className="micro-label">Tail</div>
          {tailError ? (
            <span className="font-mono text-[10px] text-destructive">
              {logErrorMessage(tailError)}
            </span>
          ) : null}
        </div>
        <MicroLog lines={dump.microLog} />
      </div>
    </div>
  );
}

function CaptureBody({
  capture,
  loading,
}: {
  capture: CaptureState;
  loading: boolean;
}) {
  if (capture.status === 'ready') {
    return <HotDumpBody dump={capture.dump} />;
  }

  if (loading) {
    return <CaptureStatus title="Loading capture" detail="Polling candump-latest…" />;
  }

  switch (capture.status) {
    case 'empty':
      return (
        <CaptureStatus
          title="No harness candump"
          detail="Link meters below still track host rx/tx when the Pi is up."
        />
      );
    case 'unavailable': {
      const showBanner = shouldShowLogErrorBanner(capture.error);
      if (showBanner) {
        return (
          <p className="text-sm text-destructive">{logErrorMessage(capture.error)}</p>
        );
      }
      return (
        <CaptureStatus
          title={capture.error.kind === 'no_endpoint' ? 'No gateway' : 'Capture unreachable'}
          detail={
            capture.error.kind === 'no_endpoint'
              ? 'Wireframe — no Chappe HTTP endpoint configured. Header CAN0 is a static profile label, not live bus state.'
              : 'Candump HTTP failed. Link meters still sample host metrics when Chappe is up.'
          }
        />
      );
    }
    default: {
      const _exhaustive: never = capture;
      return _exhaustive;
    }
  }
}

export function CanBusSpectrumPanel({ active = true }: CanBusSpectrumPanelProps) {
  const { capture, loading, linkActivity } = useCanTrafficSpectrum({ active });
  const ready = capture.status === 'ready';

  return (
    <Card
      variant="panel"
      className={cn(
        '@container/card flex w-full flex-col',
        dashboardPanelCardClassName,
      )}
      data-testid="overview-can-bus-panel"
    >
      <CardHeader className="shrink-0">
        <CardTitle>CAN bus</CardTitle>
        <CardDescription>
          {presenceCopy(capture)}
          {ready
            ? ` · ${capture.dump.parsedFrames.toLocaleString()} frames · ${formatHz(capture.dump.sessionApproxHz)} Hz`
            : ' · candump-latest'}
        </CardDescription>
        <CardAction>
          <div className="flex items-center gap-3">
            <LiveChip live={capture.live} />
            <Link
              to={LOGS_CAN_HREF}
              className="font-mono text-[10px] uppercase tracking-[0.14em] text-accent outline-none transition-colors duration-150 hover:text-accent-dim focus-visible:ring-1 focus-visible:ring-ring"
            >
              Open Logs · CAN
            </Link>
          </div>
        </CardAction>
      </CardHeader>
      <CardContent className="flex flex-col gap-2.5 px-2 pb-3 sm:px-4">
        <CaptureBody capture={capture} loading={loading} />
        <LinkActivityChart samples={linkActivity} />
      </CardContent>
    </Card>
  );
}
