import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from '@/components/ui/card';
import { StatusBadge } from '@/components/dashboard/metrics/status-badge';
import { dashboardPanelCardClassName } from '@/components/dashboard/layout/constants';
import { useCanTrafficSpectrum } from '@/hooks/use-can-traffic-spectrum';
import {
  logErrorMessage,
  shouldShowLogErrorBanner,
} from '@/lib/log-api';
import { cn } from '@/lib/utils';
import type {
  CanIdBand,
  CanLiveChip,
  CanTrafficSpectrum,
  HzSample,
  InterfacePartition,
  MicroLogLine,
} from '@/lib/can-traffic-spectrum';

type CanBusSpectrumPanelProps = {
  active: boolean;
};

function spectrumDescription(spectrum: CanTrafficSpectrum): string {
  if (spectrum.source === 'empty') {
    return 'No harness candump yet';
  }
  if (spectrum.source === 'unavailable') {
    return 'CAN capture unavailable';
  }
  const rate =
    spectrum.sessionApproxHz == null ? 'rate n/a' : `${spectrum.sessionApproxHz.toFixed(1)} Hz`;
  const freshness = spectrum.presence === 'stale' ? 'capture unchanged' : 'capture live';
  return `${spectrum.parsedFrames.toLocaleString()} frames · ${rate} · ${freshness}`;
}

export function CanBusLiveChip({ status }: { status: CanLiveChip }) {
  const tone = status.warn ? 'warning' : status.canState ? 'healthy' : 'muted';
  const label = status.iface
    ? `${status.iface} ${status.canState ?? 'UNKNOWN'}`
    : 'CAN OFFLINE';
  return <StatusBadge label={label} tone={tone} />;
}

function CanIdHistogram({ bands }: { bands: CanIdBand[] }) {
  if (bands.length === 0) {
    return <p className="font-mono text-[10px] uppercase tracking-[0.14em] text-muted-foreground">No ID bands</p>;
  }

  return (
    <div className="space-y-1.5" aria-label="CAN ID traffic share">
      {bands.map((band, index) => (
        <div
          key={band.canId}
          className="grid grid-cols-[3.25rem_minmax(0,1fr)_2.5rem] items-center gap-2 font-mono text-[10px]"
        >
          <span className="truncate text-muted-foreground">{band.canId}</span>
          <div className="h-2 overflow-hidden rounded-[1px] bg-surface-0">
            <div
              className={cn('h-full', index === 0 ? 'bg-accent' : 'bg-muted-foreground/50')}
              style={{ width: `${band.share * 100}%` }}
            />
          </div>
          <span className="text-right tabular-nums text-muted-foreground">
            {Math.round(band.share * 100)}%
          </span>
        </div>
      ))}
    </div>
  );
}

function CanRateSparkline({ samples }: { samples: HzSample[] }) {
  if (samples.length === 0) {
    return <p className="font-mono text-[10px] uppercase tracking-[0.14em] text-muted-foreground">No rate samples</p>;
  }

  const max = Math.max(...samples.map((sample) => sample.hz), 1);
  const min = Math.min(...samples.map((sample) => sample.hz));
  const range = Math.max(max - min, 1);
  const points = samples
    .map((sample, index) => {
      const x = samples.length === 1 ? 0 : (index / (samples.length - 1)) * 100;
      const y = 30 - ((sample.hz - min) / range) * 24;
      return `${x},${y}`;
    })
    .join(' ');
  const latest = samples[samples.length - 1];

  return (
    <div className="space-y-1">
      <svg
        className="h-12 w-full overflow-visible"
        viewBox="0 0 100 32"
        preserveAspectRatio="none"
        role="img"
        aria-label="CAN bus rate sparkline"
      >
        <polyline
          fill="none"
          points={points}
          stroke="currentColor"
          strokeWidth="1.5"
          vectorEffect="non-scaling-stroke"
          className="text-info"
        />
      </svg>
      <div className="flex items-center justify-between font-mono text-[10px] uppercase tracking-[0.14em] text-muted-foreground">
        <span>Hz / 2s</span>
        <span className="tabular-nums">{latest?.hz.toFixed(1)} Hz</span>
      </div>
    </div>
  );
}

function CanInterfacePartition({ partitions }: { partitions: InterfacePartition[] }) {
  if (partitions.length === 0) {
    return null;
  }

  return (
    <div className="space-y-1.5" aria-label="CAN interface traffic share">
      <div className="flex h-1.5 overflow-hidden rounded-[1px] bg-surface-0">
        {partitions.map((partition) => (
          <div
            key={partition.name}
            className="h-full bg-info/70 first:bg-accent"
            style={{ width: `${partition.share * 100}%` }}
            title={`${partition.name} ${Math.round(partition.share * 100)}%`}
          />
        ))}
      </div>
      <div className="flex flex-wrap gap-x-3 gap-y-1 font-mono text-[10px] uppercase tracking-[0.14em] text-muted-foreground">
        {partitions.map((partition) => (
          <span key={partition.name}>
            {partition.name} {Math.round(partition.share * 100)}%
          </span>
        ))}
      </div>
    </div>
  );
}

function CandumpMicroLog({ lines }: { lines: MicroLogLine[] }) {
  return (
    <div className="min-h-0 overflow-hidden rounded-[4px] border border-line bg-surface-0">
      <div className="grid grid-cols-[3.25rem_2.25rem_3.5rem_minmax(4rem,0.8fr)_minmax(3rem,0.6fr)_minmax(0,1.5fr)] gap-2 border-b border-line px-2 py-1.5 font-mono text-[9px] uppercase tracking-[0.14em] text-muted-foreground">
        <span>Δt</span>
        <span>if</span>
        <span>id</span>
        <span>joint</span>
        <span>comm</span>
        <span>data</span>
      </div>
      {lines.length === 0 ? (
        <div className="px-2 py-5 text-center font-mono text-[10px] text-muted-foreground">
          No frames in tail.
        </div>
      ) : (
        <div className="max-h-44 overflow-hidden">
          {lines.map((line) => (
            <div
              key={`${line.lineNo}-${line.offsetS}-${line.canId}`}
              className="grid grid-cols-[3.25rem_2.25rem_3.5rem_minmax(4rem,0.8fr)_minmax(3rem,0.6fr)_minmax(0,1.5fr)] gap-2 border-b border-line/60 px-2 py-1 font-mono text-[10px] last:border-b-0"
            >
              <span className="truncate tabular-nums text-muted-foreground">
                {line.offsetS.toFixed(3)}
              </span>
              <span className="truncate">{line.iface}</span>
              <span className="truncate">{line.canId}</span>
              <span className="truncate">{line.joint ?? '—'}</span>
              <span className="truncate">{line.commTypeName ?? '—'}</span>
              <span className="truncate text-muted-foreground">{line.dataHead || '—'}</span>
            </div>
          ))}
        </div>
      )}
    </div>
  );
}

export function CanBusSpectrumPanel({ active }: CanBusSpectrumPanelProps) {
  const spectrum = useCanTrafficSpectrum({ active, sessionId: 'latest' });
  const showError = shouldShowLogErrorBanner(
    spectrum.errorKind === null ? null : { kind: spectrum.errorKind },
  );

  return (
    <Card
      variant="panel"
      className={cn('@container/card flex h-full min-h-[20rem] flex-col', dashboardPanelCardClassName)}
      data-testid="overview-can-bus-panel"
    >
      <CardHeader className="shrink-0 gap-2">
        <div className="flex items-start justify-between gap-3">
          <div className="min-w-0">
            <CardTitle>CAN BUS</CardTitle>
            <CardDescription>{spectrumDescription(spectrum)}</CardDescription>
          </div>
          <CanBusLiveChip status={spectrum.live} />
        </div>
      </CardHeader>
      <CardContent className="flex min-h-0 flex-1 flex-col gap-4">
        {showError ? (
          <p className="text-xs text-destructive">
            {logErrorMessage({ kind: spectrum.errorKind! })}
          </p>
        ) : null}
        {spectrum.source === 'empty' ? (
          <div className="flex min-h-[16rem] flex-1 items-center justify-center rounded-[4px] border border-line bg-surface-0 px-4 text-center font-mono text-[10px] uppercase tracking-[0.14em] text-muted-foreground">
            No harness candump yet
          </div>
        ) : spectrum.source === 'unavailable' ? (
          <div className="flex min-h-[16rem] flex-1 items-center justify-center rounded-[4px] border border-line bg-surface-0 px-4 text-center font-mono text-[10px] uppercase tracking-[0.14em] text-muted-foreground">
            CAN capture unavailable
          </div>
        ) : (
          <>
            <div className="grid grid-cols-1 gap-4 @md/card:grid-cols-2">
              <section className="space-y-2">
                <div className="font-mono text-[10px] uppercase tracking-[0.14em] text-muted-foreground">
                  ID share
                </div>
                <CanIdHistogram bands={spectrum.bands} />
              </section>
              <section className="space-y-2">
                <div className="font-mono text-[10px] uppercase tracking-[0.14em] text-muted-foreground">
                  Bus rate
                </div>
                <CanRateSparkline samples={spectrum.rateHz} />
              </section>
            </div>
            <CanInterfacePartition partitions={spectrum.partitions} />
            <CandumpMicroLog lines={spectrum.microLog} />
          </>
        )}
        <div className="flex items-center justify-between gap-3 font-mono text-[10px] uppercase tracking-[0.14em] text-muted-foreground">
          <span>{spectrum.presence === 'stale' ? 'Snapshot stale' : 'Capture inspection'}</span>
          <a className="text-info hover:underline" href={spectrum.logsCanHref}>
            Open Logs → CAN
          </a>
        </div>
      </CardContent>
    </Card>
  );
}
import { Link } from 'react-router-dom';

import { dashboardPanelCardClassName } from '@/components/dashboard/layout/constants';
import { useCanTrafficSpectrum } from '@/hooks/use-can-traffic-spectrum';
import {
  type CanIdBand,
  type CanLiveChip,
  type CanTrafficSpectrum,
  type HzSample,
  type InterfacePartition,
  type MicroLogLine,
} from '@/lib/can-traffic-spectrum';
import { logErrorMessage, shouldShowLogErrorBanner } from '@/lib/log-api';
import { cn } from '@/lib/utils';
import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from '@/components/ui/card';

type CanBusSpectrumPanelProps = {
  active?: boolean;
};

function presenceCopy(spectrum: CanTrafficSpectrum): string {
  if (spectrum.source === 'unavailable') {
    return 'Capture unavailable';
  }
  if (spectrum.source === 'empty') {
    return 'No harness candump yet';
  }
  if (spectrum.presence === 'stale') {
    return 'Hot dump unchanged';
  }
  return 'Hot dump updating';
}

function formatHz(value: number | null): string {
  if (value == null || !Number.isFinite(value)) {
    return 'n/a';
  }
  return `${value.toFixed(1)} Hz`;
}

function LiveChip({ live }: { live: CanLiveChip }) {
  const label = live.iface ?? 'can';
  const state = live.canState?.length ? live.canState : '—';
  return (
    <span
      className={cn(
        'inline-flex items-center gap-1.5 font-mono text-[10px] uppercase tracking-[0.14em]',
        live.warn ? 'text-warning' : 'text-muted-foreground',
      )}
      data-testid="can-live-chip"
    >
      <span
        className={cn('led', live.warn ? 'led-accent' : 'led-ok')}
        aria-hidden
      />
      {label} · {state}
    </span>
  );
}

function IdHistogram({ bands }: { bands: CanIdBand[] }) {
  if (bands.length === 0) {
    return (
      <p className="text-sm text-muted-foreground">No ID spectrum yet.</p>
    );
  }
  return (
    <div className="flex min-h-0 flex-col gap-1" data-testid="can-id-histogram">
      {bands.slice(0, 8).map((band) => (
        <div key={band.canId} className="grid grid-cols-[5.5rem_1fr_2.5rem] items-center gap-2">
          <span className="truncate font-mono text-[11px] text-foreground">{band.canId}</span>
          <div className="h-1.5 overflow-hidden rounded-[2px] bg-surface-2">
            <div
              className="h-full bg-info"
              style={{ width: `${Math.max(2, band.share * 100)}%` }}
            />
          </div>
          <span className="text-right font-mono text-[10px] text-muted-foreground">
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
      <div className="flex h-10 items-end font-mono text-[10px] text-muted-foreground">
        rate trail —
      </div>
    );
  }
  const max = Math.max(...samples.map((s) => s.hz), 1);
  const width = 120;
  const height = 36;
  const points = samples
    .map((sample, index) => {
      const x = (index / (samples.length - 1)) * width;
      const y = height - (sample.hz / max) * (height - 2) - 1;
      return `${x},${y}`;
    })
    .join(' ');
  return (
    <svg
      viewBox={`0 0 ${width} ${height}`}
      className="h-10 w-full text-accent"
      data-testid="can-rate-sparkline"
      aria-hidden
    >
      <polyline
        fill="none"
        stroke="currentColor"
        strokeWidth="1.25"
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
    <div className="flex flex-wrap gap-2 font-mono text-[10px] uppercase tracking-[0.14em] text-muted-foreground">
      {partitions.map((part) => (
        <span key={part.name}>
          {part.name} {part.frameCount}
          {part.approxHz != null ? ` · ${part.approxHz.toFixed(0)}Hz` : ''}
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
      className="min-h-0 flex-1 overflow-auto rounded-[4px] border border-line bg-surface-0"
      data-testid="can-micro-log"
    >
      <div className="grid grid-cols-[3.25rem_2.25rem_3.5rem_minmax(0,1fr)_minmax(0,1.2fr)] gap-1 border-b border-line px-2 py-1 font-mono text-[10px] uppercase tracking-[0.14em] text-muted-foreground">
        <span>Δt</span>
        <span>if</span>
        <span>id</span>
        <span>joint</span>
        <span>data</span>
      </div>
      <ul className="divide-y divide-line">
        {[...lines].reverse().map((line) => (
          <li
            key={`${line.lineNo}-${line.canId}`}
            className="grid grid-cols-[3.25rem_2.25rem_3.5rem_minmax(0,1fr)_minmax(0,1.2fr)] gap-1 px-2 py-0.5 font-mono text-[11px] text-foreground"
          >
            <span className="text-muted-foreground">{line.offsetS.toFixed(3)}</span>
            <span>{line.iface.replace(/^can/, '')}</span>
            <span>{line.canId}</span>
            <span className="truncate text-muted-foreground">
              {line.joint ?? line.commTypeName ?? '—'}
            </span>
            <span className="truncate text-muted-foreground">{line.dataHead}</span>
          </li>
        ))}
      </ul>
    </div>
  );
}

export function CanBusSpectrumPanel({ active = true }: CanBusSpectrumPanelProps) {
  const spectrum = useCanTrafficSpectrum({ active });
  const showError = shouldShowLogErrorBanner(spectrum.errorKind);

  return (
    <Card
      variant="panel"
      className={cn(
        '@container/card flex h-full min-h-[20rem] flex-col',
        dashboardPanelCardClassName,
      )}
      data-testid="overview-can-bus-panel"
    >
      <CardHeader className="shrink-0 gap-2">
        <div className="flex items-start justify-between gap-3">
          <div className="min-w-0">
            <CardTitle>CAN bus</CardTitle>
            <CardDescription>
              {presenceCopy(spectrum)}
              {spectrum.source === 'hot-dump'
                ? ` · ${spectrum.parsedFrames} frames · ${formatHz(spectrum.sessionApproxHz)}`
                : ''}
            </CardDescription>
          </div>
          <LiveChip live={spectrum.live} />
        </div>
      </CardHeader>
      <CardContent className="flex min-h-0 flex-1 flex-col gap-3 px-2 pb-4 sm:px-4">
        {showError ? (
          <p className="text-sm text-destructive">
            {logErrorMessage(spectrum.errorKind!)}
          </p>
        ) : null}

        {spectrum.source === 'empty' && !showError ? (
          <div className="flex min-h-[8rem] flex-1 flex-col justify-center gap-2 rounded-[4px] border border-line bg-surface-0 px-3 py-4">
            <p className="text-sm text-foreground">No harness candump yet</p>
            <p className="text-sm text-muted-foreground">
              Bench captures write candump-latest.log. Live controller state still
              shows above when host metrics are flowing.
            </p>
          </div>
        ) : (
          <>
            <div className="grid gap-3 @md/card:grid-cols-[1.4fr_0.8fr]">
              <IdHistogram bands={spectrum.bands} />
              <div className="flex flex-col gap-1">
                <span className="font-mono text-[10px] uppercase tracking-[0.14em] text-muted-foreground">
                  Bus rate
                </span>
                <RateSparkline samples={spectrum.rateHz} />
              </div>
            </div>
            <InterfaceStrip partitions={spectrum.partitions} />
            <MicroLog lines={spectrum.microLog} />
          </>
        )}

        <div className="flex shrink-0 items-center justify-between gap-2 pt-1">
          <Link
            to={spectrum.logsCanHref}
            className="font-mono text-[10px] uppercase tracking-[0.14em] text-accent hover:underline"
          >
            Open Logs → CAN
          </Link>
          {spectrum.live.txErrorCount != null || spectrum.live.rxErrorCount != null ? (
            <span className="font-mono text-[10px] text-muted-foreground">
              err tx {spectrum.live.txErrorCount ?? '—'} / rx{' '}
              {spectrum.live.rxErrorCount ?? '—'}
            </span>
          ) : null}
        </div>
      </CardContent>
    </Card>
  );
}
