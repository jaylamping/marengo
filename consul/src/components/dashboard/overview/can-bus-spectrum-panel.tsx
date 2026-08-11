import { Link } from 'react-router-dom';

import { dashboardPanelCardClassName } from '@/components/dashboard/layout/constants';
import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from '@/components/ui/card';
import { useCanTrafficSpectrum } from '@/hooks/use-can-traffic-spectrum';
import type {
  CanIdBand,
  CanLiveChip,
  CanTrafficSpectrum,
  HzSample,
  InterfacePartition,
  MicroLogLine,
} from '@/lib/can-traffic-spectrum';
import { logErrorMessage, shouldShowLogErrorBanner } from '@/lib/log-api';
import { cn } from '@/lib/utils';

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
    return <p className="text-sm text-muted-foreground">No ID spectrum yet.</p>;
  }
  return (
    <div className="flex min-h-0 flex-col gap-1" data-testid="can-id-histogram">
      {bands.slice(0, 8).map((band) => (
        <div
          key={band.canId}
          className="grid grid-cols-[5.5rem_1fr_2.5rem] items-center gap-2"
        >
          <span className="truncate font-mono text-[11px] text-foreground">
            {band.canId}
          </span>
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
