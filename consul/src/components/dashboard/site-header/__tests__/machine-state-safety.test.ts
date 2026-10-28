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

  it('shows RUNTIME SILENT, not CONNECTING, when RobotState has gone stale', () => {
    const state = resolveMachineState(true, false, null, null, null, true);
    expect(state.label).toBe('RUNTIME SILENT');
    expect(resolveMachineState(true, false, null, null, null, false).label).toBe(
      'CONNECTING',
    );
  });

  it('lets a SafetyState E-STOP or latched fault outrank stale and connecting', () => {
    for (const stale of [true, false]) {
      expect(
        resolveMachineState(true, false, null, null, {
          hardwareEstopAsserted: true,
          faultLatched: false,
        }, stale).label,
      ).toBe('E-STOP');
      expect(
        resolveMachineState(true, false, null, null, {
          hardwareEstopAsserted: false,
          faultLatched: true,
        }, stale).label,
      ).toBe('FAULT LATCHED');
    }
  });

  it('still reports a failed gateway link as CHAPPE ERR', () => {
    expect(
      resolveMachineState(true, false, null, 'boom', {
        hardwareEstopAsserted: false,
        faultLatched: true,
      }, true).label,
    ).toBe('CHAPPE ERR');
  });
});
