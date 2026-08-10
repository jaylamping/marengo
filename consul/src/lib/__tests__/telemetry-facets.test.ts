import { describe, expect, it } from 'vitest';

import { resolveTelemetryReferenceFacet } from '@/lib/telemetry-facets';

describe('resolveTelemetryReferenceFacet (wire-gated)', () => {
  it('returns unknown when homing_state is absent on the joint', () => {
    expect(
      resolveTelemetryReferenceFacet({
        name: 'right_shoulder_pitch',
        position: 1.2,
        fault: 0,
      }),
    ).toBe('unknown');
  });

  it('returns unknown for null/undefined joint (offline placeholder)', () => {
    expect(resolveTelemetryReferenceFacet(null)).toBe('unknown');
    expect(resolveTelemetryReferenceFacet(undefined)).toBe('unknown');
  });

  it('returns unknown for UNSPECIFIED / 0 wire values (old runtime gate)', () => {
    expect(
      resolveTelemetryReferenceFacet({
        name: 'right_shoulder_pitch',
        homingState: 0,
      }),
    ).toBe('unknown');
    expect(
      resolveTelemetryReferenceFacet({
        name: 'right_shoulder_pitch',
        homing_state: 'UNSPECIFIED',
      }),
    ).toBe('unknown');
  });

  it('never treats browser zeroed/localStorage flags as Ready', () => {
    expect(
      resolveTelemetryReferenceFacet({
        name: 'right_shoulder_pitch',
        // no wire homing field — localStorage-style hint must be ignored
        zeroed: true,
        ready: true,
      }),
    ).toBe('unknown');
  });

  it('maps Verified wire values to ready when present', () => {
    expect(
      resolveTelemetryReferenceFacet({
        name: 'right_shoulder_pitch',
        homingState: 3, // JointHomingState.VERIFIED (proto ordinal reserved)
      }),
    ).toBe('ready');
    expect(
      resolveTelemetryReferenceFacet({
        name: 'right_shoulder_pitch',
        homingState: 'VERIFIED',
      }),
    ).toBe('ready');
  });

  it('maps non-Verified wire values to not_ready', () => {
    expect(
      resolveTelemetryReferenceFacet({
        name: 'right_shoulder_pitch',
        homingState: 'UNHOMED',
      }),
    ).toBe('not_ready');
    expect(
      resolveTelemetryReferenceFacet({
        name: 'right_shoulder_pitch',
        homingState: 1,
      }),
    ).toBe('not_ready');
  });
});
