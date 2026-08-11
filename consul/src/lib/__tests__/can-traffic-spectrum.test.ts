import { describe, expect, it } from 'vitest';

import type { CandumpFrameDto, CandumpSummaryDto } from '@/lib/log-api';
import {
  ACTIVITY_CAP,
  STALE_AFTER_MS,
  appendLinkActivity,
  candumpTailOffset,
  captureFingerprint,
  foldCaptureState,
  projectMicroLog,
  readCanLiveChip,
  share01,
  type CanLiveChip,
  type CaptureState,
} from '@/lib/can-traffic-spectrum';

const emptyLive: CanLiveChip = {
  iface: null,
  canState: null,
  warn: false,
  rxBytesPerSec: null,
  txBytesPerSec: null,
  txErrorCount: null,
  rxErrorCount: null,
};

function makeSummary(partial: Partial<CandumpSummaryDto> = {}): CandumpSummaryDto {
  return {
    parsed_frames: 100,
    total_lines: 100,
    source_bytes: 4096,
    duration_s: 2.5,
    approx_hz: 40,
    interfaces: [{ name: 'can0', parsed_frames: 100, approx_hz: 40 }],
    top_ids: [
      { can_id: '0x001', count: 60 },
      { can_id: '0x002', count: 40 },
    ],
    ...partial,
  };
}

function makeFrame(partial: Partial<CandumpFrameDto> = {}): CandumpFrameDto {
  return {
    delta_s: 1,
    offset_s: 1,
    interface: 'can0',
    can_id: '0x001',
    data: 'AABBCCDD',
    line_no: 1,
    joint: 'right_shoulder_pitch',
    comm_type_name: 'MIT',
    ...partial,
  };
}

describe('can-traffic-spectrum', () => {
  it('normalizes shares', () => {
    expect(share01(25, 100)).toBe(0.25);
    expect(share01(0, 0)).toBe(0);
  });

  it('computes a last-N candump page offset', () => {
    expect(candumpTailOffset(109_259, 48)).toBe(109_211);
    expect(candumpTailOffset(10, 48)).toBe(0);
    expect(candumpTailOffset(0, 48)).toBe(0);
  });

  it('treats empty dump as empty, not an error', () => {
    const capture = foldCaptureState({
      summaryResult: {
        ok: true,
        data: makeSummary({ parsed_frames: 0, top_ids: [], interfaces: [] }),
      },
      pageResult: null,
      live: emptyLive,
      previous: null,
      nowMs: 1_000,
    });
    expect(capture).toEqual({ status: 'empty', live: emptyLive });
  });

  it('builds bands and micro-log from a hot dump', () => {
    const capture = foldCaptureState({
      summaryResult: { ok: true, data: makeSummary() },
      pageResult: {
        ok: true,
        data: {
          frames: [makeFrame({ line_no: 1 }), makeFrame({ line_no: 2, can_id: '0x002' })],
          total: 100,
        },
      },
      live: emptyLive,
      previous: null,
      nowMs: 2_000,
    });
    expect(capture.status).toBe('ready');
    if (capture.status !== 'ready') return;
    expect(capture.dump.freshness).toBe('live');
    expect(capture.dump.bands[0]).toMatchObject({ canId: '0x001', count: 60, share: 0.6 });
    expect(capture.dump.partitions[0]?.name).toBe('can0');
    expect(capture.dump.microLog).toHaveLength(2);
    expect(capture.dump.microLog[1]?.joint).toBe('right_shoulder_pitch');
    expect(capture.dump.rateHz).toEqual([{ atMs: 2_000, hz: 40 }]);
    expect(capture.dump.tailError).toBeNull();
  });

  it('keeps summary ready when the tail page fails', () => {
    const capture = foldCaptureState({
      summaryResult: { ok: true, data: makeSummary() },
      pageResult: { ok: false, error: { kind: 'network' } },
      live: emptyLive,
      previous: null,
      nowMs: 2_000,
    });
    expect(capture.status).toBe('ready');
    if (capture.status !== 'ready') return;
    expect(capture.dump.bands).toHaveLength(2);
    expect(capture.dump.microLog).toHaveLength(0);
    expect(capture.dump.tailError?.kind).toBe('network');
  });

  it('marks unchanged fingerprints stale after the freshness window', () => {
    const first = foldCaptureState({
      summaryResult: { ok: true, data: makeSummary() },
      pageResult: {
        ok: true,
        data: { frames: [makeFrame()], total: 100 },
      },
      live: emptyLive,
      previous: null,
      nowMs: 1_000,
    });
    const fp = captureFingerprint(makeSummary(), {
      frames: [makeFrame()],
      total: 100,
    });
    expect(first.status).toBe('ready');
    if (first.status !== 'ready') return;
    expect(first.dump.fingerprint).toBe(fp);

    const stale = foldCaptureState({
      summaryResult: { ok: true, data: makeSummary() },
      pageResult: {
        ok: true,
        data: { frames: [makeFrame()], total: 100 },
      },
      live: emptyLive,
      previous: first,
      nowMs: 1_000 + STALE_AFTER_MS,
    });
    expect(stale.status).toBe('ready');
    if (stale.status !== 'ready') return;
    expect(stale.dump.freshness).toBe('stale');
    expect(stale.dump.capturedAtMs).toBe(1_000);
  });

  it('maps summary transport failure to unavailable', () => {
    const capture = foldCaptureState({
      summaryResult: { ok: false, error: { kind: 'unauthorized' } },
      pageResult: null,
      live: emptyLive,
      previous: null,
      nowMs: 1_000,
    });
    expect(capture).toEqual({
      status: 'unavailable',
      live: emptyLive,
      error: { kind: 'unauthorized' },
    });
  });

  it('projects a fixed micro-log window from the end of a page', () => {
    const frames = Array.from({ length: 30 }, (_, i) =>
      makeFrame({ line_no: i + 1, can_id: `0x${i.toString(16)}` }),
    );
    const lines = projectMicroLog(frames, 24);
    expect(lines).toHaveLength(24);
    expect(lines[0]?.lineNo).toBe(7);
    expect(lines[23]?.lineNo).toBe(30);
  });

  it('reads the live CAN chip from host metrics', () => {
    const chip = readCanLiveChip({
      network: [
        {
          name: 'can0',
          up: true,
          rxBytesPerSec: 100n,
          txBytesPerSec: 200n,
          rxErrorsTotal: 0n,
          txErrorsTotal: 0n,
          canState: 'BUS-OFF',
          canTxErrorCount: 3n,
          canRxErrorCount: 1n,
        },
      ],
    } as Parameters<typeof readCanLiveChip>[0]);
    expect(chip.iface).toBe('can0');
    expect(chip.canState).toBe('BUS-OFF');
    expect(chip.warn).toBe(true);
    expect(chip.txErrorCount).toBe(3);
  });

  it('preserves prior sparkline samples across ready rebuilds', () => {
    const previous: CaptureState = foldCaptureState({
      summaryResult: { ok: true, data: makeSummary({ approx_hz: 10 }) },
      pageResult: {
        ok: true,
        data: { frames: [makeFrame()], total: 100 },
      },
      live: emptyLive,
      previous: null,
      nowMs: 1_000,
    });
    const next = foldCaptureState({
      summaryResult: {
        ok: true,
        data: makeSummary({ approx_hz: 20, parsed_frames: 101, total_lines: 101 }),
      },
      pageResult: {
        ok: true,
        data: { frames: [makeFrame({ line_no: 2 })], total: 101 },
      },
      live: emptyLive,
      previous,
      nowMs: 3_000,
    });
    expect(next.status).toBe('ready');
    if (next.status !== 'ready') return;
    expect(next.dump.rateHz.map((s) => s.hz)).toEqual([10, 20]);
  });

  it('appends link-activity samples and caps the rolling window', () => {
    let series: ReturnType<typeof appendLinkActivity> = [];
    for (let i = 1; i <= ACTIVITY_CAP + 5; i += 1) {
      series = appendLinkActivity(series, {
        atMs: i * 1_000,
        rxBps: i,
        txBps: 0,
      });
    }
    expect(series).toHaveLength(ACTIVITY_CAP);
    expect(series[series.length - 1]?.rxBps).toBe(ACTIVITY_CAP + 5);
  });
});
