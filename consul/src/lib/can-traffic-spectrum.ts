import type { HostMetrics } from '@/gen/marengo/v1/marengo_pb';
import {
  type CandumpFrameDto,
  type CandumpSummaryDto,
  type LogApiError,
} from '@/lib/log-api';
import { canWarning } from '@/state/hostMetricsStore';

export const TOP_ID_BANDS = 12;
export const MICRO_LOG_LIMIT = 24;
export const SPARKLINE_CAP = 32;
export const ACTIVITY_CAP = 60;
export const ACTIVITY_TICK_MS = 1_000;
export const SPECTRUM_POLL_MS = 2_000;
export const TAIL_PAGE_LIMIT = 48;
export const STALE_AFTER_MS = 8_000;
export const LOGS_CAN_HREF = '/logs';

export type Share01 = number & { readonly __brand: 'Share01' };
export type SpectrumSource = 'hot-dump' | 'empty' | 'unavailable';
export type CapturePresence = 'absent' | 'stale' | 'live';

export type CanIdBand = {
  canId: string;
  count: number;
  share: Share01;
  joint?: string;
};

export type InterfacePartition = {
  name: string;
  frameCount: number;
  share: Share01;
  approxHz: number | null;
};

export type HzSample = {
  atMs: number;
  hz: number;
};

/** Rolling link throughput sample from host metrics (independent of candump). */
export type CanLinkActivitySample = {
  atMs: number;
  rxBps: number;
  txBps: number;
};

export type MicroLogLine = {
  lineNo: number;
  offsetS: number;
  iface: string;
  canId: string;
  dataHead: string;
  joint?: string;
  commTypeName?: string;
};

export type CanLiveChip = {
  iface: string | null;
  canState: string | null;
  warn: boolean;
  rxBytesPerSec: number | null;
  txBytesPerSec: number | null;
  txErrorCount: number | null;
  rxErrorCount: number | null;
};

export type CanTrafficSpectrum = {
  source: SpectrumSource;
  presence: CapturePresence;
  fingerprint: string | null;
  capturedAtMs: number;
  durationS: number;
  parsedFrames: number;
  sessionApproxHz: number | null;
  bands: CanIdBand[];
  partitions: InterfacePartition[];
  rateHz: HzSample[];
  microLog: MicroLogLine[];
  live: CanLiveChip;
  errorKind: LogApiError | null;
  logsCanHref: string;
};

export type BuildSpectrumInput = {
  summary: CandumpSummaryDto | null;
  page: { frames: CandumpFrameDto[]; total: number } | null;
  live: CanLiveChip;
  previous: CanTrafficSpectrum | null;
  nowMs: number;
  summaryError: LogApiError | null;
  pageError: LogApiError | null;
};

export function share01(numerator: number, denominator: number): Share01 {
  if (!(denominator > 0) || !(numerator >= 0) || !Number.isFinite(numerator)) {
    return 0 as Share01;
  }
  return Math.min(1, numerator / denominator) as Share01;
}

export function readCanLiveChip(metrics: HostMetrics | null): CanLiveChip {
  const iface =
    metrics?.network?.find((item) => item.name.startsWith('can')) ?? null;
  return {
    iface: iface?.name ?? null,
    canState: iface?.canState || null,
    warn: canWarning(metrics),
    rxBytesPerSec: iface != null ? Number(iface.rxBytesPerSec) : null,
    txBytesPerSec: iface != null ? Number(iface.txBytesPerSec) : null,
    txErrorCount: iface != null ? Number(iface.canTxErrorCount) : null,
    rxErrorCount: iface != null ? Number(iface.canRxErrorCount) : null,
  };
}

export function appendHzSample(
  previous: HzSample[],
  sample: HzSample,
  cap = SPARKLINE_CAP,
): HzSample[] {
  if (cap <= 0) {
    return [];
  }
  return [...previous, sample].slice(-Math.floor(cap));
}

export function appendLinkActivity(
  previous: CanLinkActivitySample[],
  sample: CanLinkActivitySample,
  cap = ACTIVITY_CAP,
): CanLinkActivitySample[] {
  if (cap <= 0) {
    return [];
  }
  const last = previous[previous.length - 1];
  if (last != null && last.atMs === sample.atMs) {
    return [...previous.slice(0, -1), sample].slice(-Math.floor(cap));
  }
  return [...previous, sample].slice(-Math.floor(cap));
}

export function seedLinkActivity(
  nowMs: number,
  live: CanLiveChip,
  count = 24,
  tickMs = ACTIVITY_TICK_MS,
): CanLinkActivitySample[] {
  const rx = live.rxBytesPerSec ?? 0;
  const tx = live.txBytesPerSec ?? 0;
  const n = Math.max(2, count);
  return Array.from({ length: n }, (_, i) => ({
    atMs: nowMs - (n - 1 - i) * tickMs,
    rxBps: rx,
    txBps: tx,
  }));
}

export function projectMicroLog(
  frames: CandumpFrameDto[],
  limit = MICRO_LOG_LIMIT,
): MicroLogLine[] {
  if (limit <= 0) {
    return [];
  }
  return frames.slice(-Math.floor(limit)).map((frame) => ({
    lineNo: frame.line_no,
    offsetS: frame.offset_s ?? frame.delta_s,
    iface: frame.interface,
    canId: frame.can_id,
    dataHead: frame.data.replace(/\s+/g, '').slice(0, 16),
    joint: frame.joint,
    commTypeName: frame.comm_type_name,
  }));
}

export function captureFingerprint(
  summary: CandumpSummaryDto | null,
  page: { frames: CandumpFrameDto[]; total: number } | null,
): string | null {
  if (!summary || summary.parsed_frames === 0) {
    return null;
  }
  const last = page?.frames[page.frames.length - 1];
  return [
    summary.parsed_frames,
    summary.total_lines,
    summary.source_bytes,
    summary.duration_s.toFixed(3),
    last?.line_no ?? 0,
    last?.can_id ?? '',
  ].join('|');
}

function buildBands(summary: CandumpSummaryDto): CanIdBand[] {
  const total = Math.max(
    summary.parsed_frames,
    summary.top_ids.reduce((acc, row) => acc + row.count, 0),
  );
  return summary.top_ids
    .filter((item) => item.count > 0)
    .slice()
    .sort((a, b) => b.count - a.count)
    .slice(0, TOP_ID_BANDS)
    .map((item) => ({
      canId: item.can_id,
      count: item.count,
      share: share01(item.count, total),
    }));
}

function buildPartitions(summary: CandumpSummaryDto): InterfacePartition[] {
  return summary.interfaces
    .filter((item) => item.parsed_frames > 0)
    .slice()
    .sort((a, b) => b.parsed_frames - a.parsed_frames)
    .map((item) => ({
      name: item.name,
      frameCount: item.parsed_frames,
      share: share01(item.parsed_frames, summary.parsed_frames),
      approxHz: item.approx_hz,
    }));
}

function estimatePageHz(frames: CandumpFrameDto[]): number | null {
  if (frames.length < 2) {
    return null;
  }
  const first = frames[0]?.offset_s ?? frames[0]?.delta_s;
  const last = frames[frames.length - 1]?.offset_s ?? frames[frames.length - 1]?.delta_s;
  const span = last - first;
  const hz = (frames.length - 1) / span;
  return span > 0 && Number.isFinite(hz) ? hz : null;
}

function idleSpectrum(
  live: CanLiveChip,
  nowMs: number,
  source: SpectrumSource,
  errorKind: LogApiError | null,
  rateHz: HzSample[],
): CanTrafficSpectrum {
  return {
    source,
    presence: 'absent',
    fingerprint: null,
    capturedAtMs: nowMs,
    durationS: 0,
    parsedFrames: 0,
    sessionApproxHz: null,
    bands: [],
    partitions: [],
    rateHz,
    microLog: [],
    live,
    errorKind,
    logsCanHref: LOGS_CAN_HREF,
  };
}

export function buildCanTrafficSpectrum(input: BuildSpectrumInput): CanTrafficSpectrum {
  const { summary, page, live, previous, nowMs, summaryError, pageError } = input;
  const transportError = summaryError ?? pageError;

  if (transportError && !summary) {
    return idleSpectrum(
      live,
      nowMs,
      'unavailable',
      transportError,
      previous?.rateHz ?? [],
    );
  }

  if (!summary || summary.parsed_frames === 0) {
    return idleSpectrum(live, nowMs, 'empty', null, previous?.rateHz ?? []);
  }

  const bands = buildBands(summary);
  const partitions = buildPartitions(summary);
  const microLog = projectMicroLog(page?.frames ?? []);
  const fingerprint = captureFingerprint(summary, page);
  const prevFp = previous?.fingerprint ?? null;
  const unchanged = fingerprint != null && fingerprint === prevFp;
  const capturedAtMs = unchanged ? (previous?.capturedAtMs ?? nowMs) : nowMs;
  const presence: CapturePresence = unchanged
    ? nowMs - capturedAtMs >= STALE_AFTER_MS
      ? 'stale'
      : 'live'
    : 'live';

  let rateHz = previous?.rateHz ?? [];
  const approxHz =
    summary.approx_hz != null && Number.isFinite(summary.approx_hz)
      ? summary.approx_hz
      : estimatePageHz(page?.frames ?? []);
  if (approxHz != null) {
    const last = rateHz[rateHz.length - 1];
    if (last == null || last.atMs !== nowMs || last.hz !== approxHz) {
      rateHz = appendHzSample(rateHz, { atMs: nowMs, hz: approxHz });
    }
  }

  return {
    source: 'hot-dump',
    presence,
    fingerprint,
    capturedAtMs,
    durationS: summary.duration_s,
    parsedFrames: summary.parsed_frames,
    sessionApproxHz: summary.approx_hz,
    bands,
    partitions,
    rateHz,
    microLog,
    live,
    errorKind: null,
    logsCanHref: LOGS_CAN_HREF,
  };
}
