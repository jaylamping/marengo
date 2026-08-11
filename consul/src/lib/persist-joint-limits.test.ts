import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

import {
  persistJointLimits,
  softLimitsWithInset,
  DEFAULT_HARD_MARGIN_RAD,
  DEFAULT_SOFT_INSET_RAD,
} from '@/lib/persist-joint-limits';

describe('softLimitsWithInset', () => {
  it('keeps ADR 0009 inset inside hard', () => {
    const { softLower, softUpper } = softLimitsWithInset(-0.5, 0.95);
    expect(softLower).toBeCloseTo(-0.5 + DEFAULT_SOFT_INSET_RAD, 6);
    expect(softUpper).toBeCloseTo(0.95 - DEFAULT_SOFT_INSET_RAD, 6);
    expect(softLower).toBeLessThan(softUpper);
  });
});

describe('persistJointLimits', () => {
  beforeEach(() => {
    vi.stubEnv('VITE_LIMIT_SYNC_URL', '');
  });

  afterEach(() => {
    vi.useRealTimers();
    vi.unstubAllEnvs();
    vi.restoreAllMocks();
  });

  it('warns when VITE_LIMIT_SYNC_URL is unset after Durable Apply', async () => {
    const fetchSpy = vi
      .spyOn(globalThis, 'fetch')
      .mockResolvedValue(new Response(JSON.stringify({ ok: true }), { status: 200 }));
    const patchConfig = vi.fn().mockResolvedValue({
      ok: true,
      message: 'Applied live limits',
      restart_required: false,
      persist_status: 'durable',
    });

    const result = await persistJointLimits(
      'right_shoulder_pitch',
      { lower: -0.5, upper: 1.2 },
      { patchConfig },
    );

    expect(result.ok).toBe(true);
    if (result.ok) {
      expect(result.localSync).toBe('missing_config');
      expect(result.message).toMatch(/VITE_LIMIT_SYNC_URL/i);
      expect(result.message).toMatch(/limit-sync-serve/i);
    }
    expect(fetchSpy).not.toHaveBeenCalled();
  });

  it('POSTs local limit-patch only when VITE_LIMIT_SYNC_URL is set', async () => {
    vi.stubEnv('VITE_LIMIT_SYNC_URL', 'http://127.0.0.1:8790');
    const fetchSpy = vi
      .spyOn(globalThis, 'fetch')
      .mockResolvedValue(new Response(JSON.stringify({ ok: true }), { status: 200 }));
    const patchConfig = vi.fn().mockResolvedValue({
      ok: true,
      message: 'Applied live limits',
      restart_required: false,
      persist_status: 'durable',
    });

    const result = await persistJointLimits(
      'right_shoulder_pitch',
      { lower: -0.5, upper: 1.2 },
      { patchConfig },
    );

    expect(result.ok).toBe(true);
    if (result.ok) {
      expect(result.localSync).toBe('ok');
    }
    expect(fetchSpy).toHaveBeenCalledWith(
      'http://127.0.0.1:8790/local/limit-patch',
      expect.objectContaining({ method: 'POST' }),
    );
  });

  it('marks opted-in local sync failed when fetch throws', async () => {
    vi.stubEnv('VITE_LIMIT_SYNC_URL', 'http://127.0.0.1:8790');
    vi.spyOn(globalThis, 'fetch').mockRejectedValue(new TypeError('Failed to fetch'));
    const patchConfig = vi.fn().mockResolvedValue({
      ok: true,
      message: 'Applied live limits',
      restart_required: false,
      persist_status: 'durable',
    });

    const result = await persistJointLimits(
      'right_shoulder_pitch',
      { lower: -0.5, upper: 1.2 },
      { patchConfig },
    );

    expect(result.ok).toBe(true);
    if (result.ok) {
      expect(result.localSync).toBe('failed');
      expect(result.message).toMatch(/Local checkout sync failed/i);
    }
  });

  it('patches hard + soft inset and syncs local only after durable', async () => {
    const patchConfig = vi.fn().mockResolvedValue({
      ok: true,
      message: 'Applied live limits',
      restart_required: false,
      persist_status: 'durable',
    });
    const localSync = vi.fn().mockResolvedValue('ok' as const);

    const result = await persistJointLimits(
      'right_shoulder_pitch',
      { lower: -0.506, upper: 1.206 },
      { patchConfig, localSync },
    );

    const hardLower = -0.506 - DEFAULT_HARD_MARGIN_RAD;
    const hardUpper = 1.206 + DEFAULT_HARD_MARGIN_RAD;
    const soft = softLimitsWithInset(hardLower, hardUpper);
    expect(result.ok).toBe(true);
    if (result.ok) {
      expect(result.lower).toBeCloseTo(hardLower, 6);
      expect(result.upper).toBeCloseTo(hardUpper, 6);
      expect(result.softLower).toBeCloseTo(soft.softLower, 6);
      expect(result.softUpper).toBeCloseTo(soft.softUpper, 6);
      expect(result.persistStatus).toBe('durable');
      expect(result.localSync).toBe('ok');
      expect(result.message).toMatch(/Local checkout synced/i);
    }
    expect(patchConfig).toHaveBeenCalledWith(
      {
        joint: 'right_shoulder_pitch',
        position_lower_rad: hardLower,
        position_upper_rad: hardUpper,
        position_soft_lower_rad: soft.softLower,
        position_soft_upper_rad: soft.softUpper,
      },
      { signal: expect.any(AbortSignal) },
    );
    expect(localSync).toHaveBeenCalled();
  });

  it('does not local-sync when persist is only pending', async () => {
    const patchConfig = vi.fn().mockResolvedValue({
      ok: true,
      message: 'Applied live',
      restart_required: false,
      persist_status: 'pending',
    });
    const localSync = vi.fn().mockResolvedValue('ok' as const);
    const result = await persistJointLimits(
      'right_elbow_pitch',
      { lower: -0.5, upper: 0.95 },
      { patchConfig, localSync },
    );
    expect(result.ok).toBe(true);
    if (result.ok) {
      expect(result.localSync).toBe('skipped');
    }
    expect(localSync).not.toHaveBeenCalled();
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
