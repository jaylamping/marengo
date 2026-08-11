import { describe, expect, it } from 'vitest';

import type { CandumpFrameDto, CandumpSummaryDto } from '@/lib/log-api';
import {
  STALE_AFTER_MS,
  buildCanTrafficSpectrum,
  captureFingerprint,
  projectMicroLog,
  readCanLiveChip,
  share01,
  type CanLiveChip,
  type CanTrafficSpectrum,
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

function summary(partial: Partial<CandumpSummaryDto> = {}): CandumpSummaryDto {
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

function frame(partial: Partial<CandumpFrameDto> = {}): CandumpFrameDto {
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

  it('treats empty dump as absent, not an error', () => {
    const spectrum = buildCanTrafficSpectrum({
      summary: summary({ parsed_frames: 0, top_ids: [], interfaces: [] }),
      page: { frames: [], total: 0 },
      live: emptyLive,
      previous: null,
      nowMs: 1_000,
      summaryError: null,
      pageError: null,
    });
    expect(spectrum.source).toBe('empty');
    expect(spectrum.presence).toBe('absent');
    expect(spectrum.errorKind).toBeNull();
    expect(spectrum.bands).toHaveLength(0);
  });

  it('builds bands and micro-log from a hot dump', () => {
    const spectrum = buildCanTrafficSpectrum({
      summary: summary(),
      page: {
        frames: [frame({ line_no: 1 }), frame({ line_no: 2, can_id: '0x002' })],
        total: 100,
      },
      live: emptyLive,
      previous: null,
      nowMs: 2_000,
      summaryError: null,
      pageError: null,
    });
    expect(spectrum.source).toBe('hot-dump');
    expect(spectrum.presence).toBe('live');
    expect(spectrum.bands[0]).toMatchObject({ canId: '0x001', count: 60, share: 0.6 });
    expect(spectrum.partitions[0]?.name).toBe('can0');
    expect(spectrum.microLog).toHaveLength(2);
    expect(spectrum.microLog[1]?.joint).toBe('right_shoulder_pitch');
    expect(spectrum.rateHz).toEqual([{ atMs: 2_000, hz: 40 }]);
  });

  it('marks unchanged fingerprints stale after the freshness window', () => {
    const first = buildCanTrafficSpectrum({
      summary: summary(),
      page: { frames: [frame()], total: 100 },
      live: emptyLive,
      previous: null,
      nowMs: 1_000,
      summaryError: null,
      pageError: null,
    });
    const fp = captureFingerprint(summary(), { frames: [frame()], total: 100 });
    expect(first.fingerprint).toBe(fp);

    const stale = buildCanTrafficSpectrum({
      summary: summary(),
      page: { frames: [frame()], total: 100 },
      live: emptyLive,
      previous: first,
      nowMs: 1_000 + STALE_AFTER_MS,
      summaryError: null,
      pageError: null,
    });
    expect(stale.presence).toBe('stale');
    expect(stale.capturedAtMs).toBe(1_000);
  });

  it('maps transport failure without summary to unavailable', () => {
    const spectrum = buildCanTrafficSpectrum({
      summary: null,
      page: null,
      live: emptyLive,
      previous: null,
      nowMs: 1_000,
      summaryError: { kind: 'unauthorized' },
      pageError: null,
    });
    expect(spectrum.source).toBe('unavailable');
    expect(spectrum.errorKind?.kind).toBe('unauthorized');
  });

  it('projects a fixed micro-log tail', () => {
    const frames = Array.from({ length: 30 }, (_, i) =>
      frame({ line_no: i + 1, can_id: `0x${i.toString(16)}` }),
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

  it('preserves prior sparkline samples across rebuilds', () => {
    const previous: CanTrafficSpectrum = {
      ...buildCanTrafficSpectrum({
        summary: summary({ approx_hz: 10 }),
        page: { frames: [frame()], total: 100 },
        live: emptyLive,
        previous: null,
        nowMs: 1_000,
        summaryError: null,
        pageError: null,
      }),
    };
    const next = buildCanTrafficSpectrum({
      summary: summary({ approx_hz: 20, parsed_frames: 101, total_lines: 101 }),
      page: { frames: [frame({ line_no: 2 })], total: 101 },
      live: emptyLive,
      previous,
      nowMs: 3_000,
      summaryError: null,
      pageError: null,
    });
    expect(next.rateHz.map((s) => s.hz)).toEqual([10, 20]);
  });
});
