import { describe, expect, it } from 'vitest';

import {
  canStartLimitListen,
  emptyBounds,
  foldPosition,
  formatJointRange,
  limitListenBlockReason,
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

  it('requires at least two distinct samples for a proposal', () => {
    let b = emptyBounds();
    b = foldPosition(b, 0.4);
    expect(proposedRangeFromBounds(b)).toBeNull();
    b = foldPosition(b, 0.4);
    expect(proposedRangeFromBounds(b)).toBeNull();
    b = foldPosition(b, 1.2);
    expect(proposedRangeFromBounds(b)).toBe('0.40–1.20');
  });

  it('gates live listen on connected and not ACTIVE', () => {
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
      limitListenBlockReason({
        connected: true,
        operationalMode: 'ACTIVE',
      }),
    ).toMatch(/Disable motors/i);
    expect(
      limitListenBlockReason({
        connected: false,
        operationalMode: null,
      }),
    ).toMatch(/Connect Chappe/);
  });
});
