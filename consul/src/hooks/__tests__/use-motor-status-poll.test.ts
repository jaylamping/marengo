// @vitest-environment jsdom
import { cleanup, renderHook } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

import {
  MOTOR_STATUS_POLL_MS,
  useMotorStatusPoll,
} from '@/hooks/use-motor-status-poll';
import { postMotorStatusPoll } from '@/lib/gateway-api';

vi.mock('@/lib/gateway-api', () => ({
  postMotorStatusPoll: vi.fn(async () => undefined),
}));

vi.mock('@/lib/chappe-config', async () => {
  const actual =
    await vi.importActual<typeof import('@/lib/chappe-config')>('@/lib/chappe-config');
  return {
    ...actual,
    isChappeLive: () => true,
  };
});

describe('useMotorStatusPoll', () => {
  beforeEach(() => {
    vi.useFakeTimers();
  });

  afterEach(() => {
    cleanup();
    vi.clearAllMocks();
    vi.useRealTimers();
  });

  it('polls immediately and again on the interval while enabled', async () => {
    renderHook(() => useMotorStatusPoll({ enabled: true }));

    await vi.advanceTimersByTimeAsync(0);
    expect(postMotorStatusPoll).toHaveBeenCalledTimes(1);

    await vi.advanceTimersByTimeAsync(MOTOR_STATUS_POLL_MS);
    expect(postMotorStatusPoll).toHaveBeenCalledTimes(2);

    await vi.advanceTimersByTimeAsync(MOTOR_STATUS_POLL_MS);
    expect(postMotorStatusPoll).toHaveBeenCalledTimes(3);
  });

  it('backs off longer after a 429', async () => {
    vi.mocked(postMotorStatusPoll)
      .mockRejectedValueOnce(new Error('motor status poll failed: 429 rate limit'))
      .mockResolvedValue(undefined);

    renderHook(() => useMotorStatusPoll({ enabled: true }));
    await vi.advanceTimersByTimeAsync(0);
    expect(postMotorStatusPoll).toHaveBeenCalledTimes(1);

    await vi.advanceTimersByTimeAsync(MOTOR_STATUS_POLL_MS);
    expect(postMotorStatusPoll).toHaveBeenCalledTimes(1);

    await vi.advanceTimersByTimeAsync(MOTOR_STATUS_POLL_MS);
    expect(postMotorStatusPoll).toHaveBeenCalledTimes(2);
  });

  it('does not poll when disabled', async () => {
    renderHook(() => useMotorStatusPoll({ enabled: false }));
    await vi.advanceTimersByTimeAsync(0);
    expect(postMotorStatusPoll).not.toHaveBeenCalled();
  });

  it('stops polling after unmount', async () => {
    const { unmount } = renderHook(() => useMotorStatusPoll({ enabled: true }));
    await vi.advanceTimersByTimeAsync(0);
    expect(postMotorStatusPoll).toHaveBeenCalledTimes(1);
    unmount();
    await vi.advanceTimersByTimeAsync(MOTOR_STATUS_POLL_MS * 2);
    expect(postMotorStatusPoll).toHaveBeenCalledTimes(1);
  });
});
