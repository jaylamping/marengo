import { afterEach, describe, expect, it, vi } from 'vitest';

import { persistJointLimits } from '@/lib/persist-joint-limits';

describe('persistJointLimits', () => {
  afterEach(() => {
    vi.useRealTimers();
  });

  it('patches motors.yaml bench bounds from exact measured numbers', async () => {
    const patchConfig = vi.fn().mockResolvedValue({
      ok: true,
      message: 'Updated right_shoulder_pitch',
      restart_required: true,
    });

    const result = await persistJointLimits(
      'right_shoulder_pitch',
      { lower: -0.506, upper: 1.206 },
      { patchConfig },
    );

    expect(result).toEqual({
      ok: true,
      lower: -0.506,
      upper: 1.206,
      restartRequired: true,
      message: 'Updated right_shoulder_pitch',
    });
    expect(patchConfig).toHaveBeenCalledWith(
      {
        joint: 'right_shoulder_pitch',
        position_lower_rad: -0.506,
        position_upper_rad: 1.206,
      },
      { signal: expect.any(AbortSignal) },
    );
  });

  it('rejects inverted or non-finite bounds without calling the gateway', async () => {
    const patchConfig = vi.fn();
    const result = await persistJointLimits(
      'right_shoulder_roll',
      { lower: 1, upper: 0 },
      { patchConfig },
    );
    expect(result.ok).toBe(false);
    expect(patchConfig).not.toHaveBeenCalled();
  });

  it('surfaces a gateway/auth failure when patchConfig returns null', async () => {
    const patchConfig = vi.fn().mockResolvedValue(null);
    const result = await persistJointLimits(
      'right_shoulder_roll',
      { lower: -1.57, upper: 1.57 },
      { patchConfig },
    );
    expect(result.ok).toBe(false);
    if (!result.ok) {
      expect(result.message).toMatch(/gateway|auth|token|Chappe|timed out/i);
    }
  });

  it('surfaces gateway-declared ok:false without treating it as success', async () => {
    const patchConfig = vi.fn().mockResolvedValue({
      ok: false,
      message: 'joint unknown',
      restart_required: false,
    });
    const result = await persistJointLimits(
      'missing_joint',
      { lower: -1, upper: 1 },
      { patchConfig },
    );
    expect(result).toEqual({ ok: false, message: 'joint unknown' });
  });

  it('times out a stalled patchConfig call', async () => {
    vi.useFakeTimers();
    const patchConfig = vi.fn(
      (_patch: unknown, init?: { signal?: AbortSignal }) =>
        new Promise<null>((resolve) => {
          init?.signal?.addEventListener('abort', () => resolve(null));
        }),
    );
    const pending = persistJointLimits(
      'right_shoulder_roll',
      { lower: -1, upper: 1 },
      { patchConfig, timeoutMs: 50 },
    );
    await vi.advanceTimersByTimeAsync(60);
    const result = await pending;
    expect(result.ok).toBe(false);
    if (!result.ok) {
      expect(result.message).toMatch(/timed out/i);
    }
  });
});
