import { describe, expect, it } from 'vitest';

import { resolveMachineState } from '../site-header-status-badges';

describe('resolveMachineState safety precedence', () => {
  it('shows a latched Davout fault instead of a calm DISABLED', () => {
    const state = resolveMachineState(true, true, 'DISABLED', null, {
      hardwareEstopAsserted: false,
      faultLatched: true,
    });
    expect(state.label).toBe('FAULT LATCHED');
    expect(state.textClassName).toBe('text-fault');
  });

  it('puts a hardware E-stop above every mode', () => {
    const state = resolveMachineState(true, true, 'ACTIVE', null, {
      hardwareEstopAsserted: true,
      faultLatched: true,
    });
    expect(state.label).toBe('E-STOP');
  });

  it('keeps the mode label when safety is clear or unknown', () => {
    expect(
      resolveMachineState(true, true, 'DISABLED', null, {
        hardwareEstopAsserted: false,
        faultLatched: false,
      }).label,
    ).toBe('DISABLED');
    expect(resolveMachineState(true, true, 'DISABLED', null).label).toBe('DISABLED');
  });
});
