import { describe, expect, it } from 'vitest';

import {
  canStartLimitListen,
  emptyBounds,
  foldPosition,
  formatJointRange,
  limitListenBlockReason,
  parseJointRange,
  proposedRangeFromBounds,
} from '@/lib/limit-listen';

describe('limit-listen', () => {
  it('folds positions into running min/max', () => {
    let b = emptyBounds();
    b = foldPosition(b, 0.5);
    b = foldPosition(b, -0.2);
    b = foldPosition(b, 1.1);
    expect(b.min).toBeCloseTo(-0.2);
    expect(b.max).toBeCloseTo(1.1);
    expect(b.sampleCount).toBe(3);
    expect(b.lastPosition).toBeCloseTo(1.1);
  });

  it('formats symmetric ranges with ±', () => {
    expect(formatJointRange(-1.57, 1.57)).toBe('±1.57');
  });

  it('formats asymmetric ranges with en-dash', () => {
    expect(formatJointRange(-0.9, 3.17)).toBe('−0.90–3.17');
  });

  it('parses ± and asymmetric ranges (unicode and ASCII)', () => {
    expect(parseJointRange('±1.57')).toEqual({ lower: -1.57, upper: 1.57 });
    expect(parseJointRange('−0.90–3.17')).toEqual({ lower: -0.9, upper: 3.17 });
    expect(parseJointRange('-0.90–3.17')).toEqual({ lower: -0.9, upper: 3.17 });
    expect(parseJointRange('0.40–1.20')).toEqual({ lower: 0.4, upper: 1.2 });
    expect(parseJointRange('not-a-range')).toBeNull();
  });

  it('round-trips formatJointRange through parseJointRange', () => {
    for (const [lo, hi] of [
      [-1.57, 1.57],
      [-0.9, 3.17],
      [0.4, 1.2],
      [-0.5, 1.2],
    ] as const) {
      const parsed = parseJointRange(formatJointRange(lo, hi));
      expect(parsed).not.toBeNull();
      expect(parsed!.lower).toBeCloseTo(lo, 5);
      expect(parsed!.upper).toBeCloseTo(hi, 5);
    }
  });

  it('requires enough samples and span before proposing a range', () => {
    let b = emptyBounds();
    b = foldPosition(b, 0.4);
    expect(proposedRangeFromBounds(b)).toBeNull();
    b = foldPosition(b, 0.4);
    expect(proposedRangeFromBounds(b)).toBeNull();
    b = foldPosition(b, 0.41);
    b = foldPosition(b, 0.42);
    b = foldPosition(b, 0.43);
    // Five samples but span << MIN_PROPOSAL_SPAN_RAD
    expect(proposedRangeFromBounds(b)).toBeNull();
    b = foldPosition(b, 1.2);
    expect(proposedRangeFromBounds(b)).toBe('0.40–1.20');
  });

  it('gates live listen on connected and known non-ACTIVE mode', () => {
    expect(
      canStartLimitListen({
        connected: true,
        operationalMode: 'DISABLED',
      }),
    ).toBe(true);
    expect(
      canStartLimitListen({
        connected: true,
        operationalMode: 'READY',
      }),
    ).toBe(true);
    expect(
      canStartLimitListen({
        connected: true,
        operationalMode: 'ACTIVE',
      }),
    ).toBe(false);
    expect(
      canStartLimitListen({
        connected: true,
        operationalMode: null,
      }),
    ).toBe(false);
    expect(
      limitListenBlockReason({
        connected: true,
        operationalMode: 'ACTIVE',
      }),
    ).toMatch(/Disable motors/i);
    expect(
      limitListenBlockReason({
        connected: true,
        operationalMode: null,
      }),
    ).toMatch(/Waiting for operational mode/i);
    expect(
      limitListenBlockReason({
        connected: false,
        operationalMode: null,
      }),
    ).toMatch(/Connect Chappe/);
  });
});
