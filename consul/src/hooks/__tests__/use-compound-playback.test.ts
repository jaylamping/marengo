// @vitest-environment jsdom
import { act, renderHook } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

import { useCompoundPlayback } from '@/hooks/use-compound-playback';
import { postEnableCommand, postTestingMitCommandBatch } from '@/lib/gateway-api';
import { useCompoundStore } from '@/state/compoundStore';
import { useRobotStore } from '@/state/robotStore';
import { useTestingStore } from '@/state/testingStore';

vi.mock('@/hooks/use-config-snapshot', () => ({
  useConfigSnapshot: () => ({ data: null }),
}));
vi.mock('@/lib/gateway-api', () => ({
  postEnableCommand: vi.fn(),
  postTestingMitCommandBatch: vi.fn(),
}));
// Live Wave is gated by sign-off; these tests drive the runner, not the gate.
vi.mock('@/data/compound-tests', async (importOriginal) => ({
  ...(await importOriginal<typeof import('@/data/compound-tests')>()),
  WAVE_POSE_GCOMP_SIGNED: true,
}));

beforeEach(() => {
  vi.useFakeTimers();
  vi.mocked(postEnableCommand).mockReset().mockResolvedValue(undefined);
  vi.mocked(postTestingMitCommandBatch).mockReset().mockResolvedValue(undefined);
  useRobotStore.setState({ connected: true, operationalMode: 'ACTIVE', robotState: null });
  useCompoundStore.getState().setSelectedPresetId('wave');
});

let unmountHook: (() => void) | null = null;

function renderPlayback() {
  const rendered = renderHook(() => useCompoundPlayback());
  unmountHook = () => {
    act(() => rendered.result.current.stopRunner({ returnHome: false }));
    rendered.unmount();
  };
  return rendered;
}

afterEach(() => {
  // Stop and unmount inside the test so no React work outlives the jsdom environment.
  unmountHook?.();
  unmountHook = null;
  useCompoundStore.getState().setSelectedPresetId(null);
  useTestingStore.setState({ dryRun: true, isRunning: false });
  vi.useRealTimers();
});

async function runFor(ms: number) {
  await act(async () => {
    await vi.advanceTimersByTimeAsync(ms);
  });
}

describe('compound playback', () => {
  it('a Dry Run run never posts, even if Dry Run is unticked mid-run (UI-03)', async () => {
    useTestingStore.setState({ dryRun: true });
    const { result } = renderPlayback();
    act(() => result.current.startRunner());
    expect(useCompoundStore.getState().isRunning).toBe(true);
    act(() => useTestingStore.setState({ dryRun: false }));
    await runFor(15_000);
    expect(postTestingMitCommandBatch).not.toHaveBeenCalled();
  });

  it('a Disable stops the run interval: nothing posts afterwards (UI-04)', async () => {
    useTestingStore.setState({ dryRun: false });
    const { result } = renderPlayback();
    act(() => result.current.startRunner());
    await runFor(0);
    const before = vi.mocked(postTestingMitCommandBatch).mock.calls.length;
    expect(before).toBe(1);
    await act(async () => {
      await useTestingStore.getState().disable();
    });
    await runFor(15_000);
    expect(vi.mocked(postTestingMitCommandBatch).mock.calls.length).toBe(before);
  });

  it('Wave Loop posts the next wave when one ends (UI-14)', async () => {
    useTestingStore.setState({ dryRun: false });
    expect(useCompoundStore.getState().loop).toBe(true);
    const { result } = renderPlayback();
    act(() => result.current.startRunner());
    const wavePosts = () =>
      vi.mocked(postTestingMitCommandBatch).mock.calls.filter(([batch]) =>
        batch.joints[0]?.name.startsWith('wave:'),
      ).length;
    // Raise plus two 140 s waves.
    await runFor(320_000);
    expect(wavePosts()).toBeGreaterThanOrEqual(2);
    expect(useCompoundStore.getState().isRunning).toBe(true);
  });

  it('a Dry Run of a settle-advance preset finishes on time (UI-15)', async () => {
    useTestingStore.setState({ dryRun: true });
    act(() => useCompoundStore.getState().setSelectedPresetId('arm_out_forward'));
    const { result } = renderPlayback();
    act(() => result.current.startRunner());
    expect(useCompoundStore.getState().isRunning).toBe(true);
    await runFor(10_000);
    expect(useCompoundStore.getState().isRunning).toBe(false);
    expect(postTestingMitCommandBatch).not.toHaveBeenCalled();
  });

  it('a second Start replaces the run interval instead of adding one (P2B-01)', async () => {
    useTestingStore.setState({ dryRun: true });
    const { result } = renderPlayback();
    act(() => result.current.startRunner());
    const timersAfterFirstStart = vi.getTimerCount();
    act(() => result.current.startRunner());
    expect(vi.getTimerCount()).toBe(timersAfterFirstStart);
  });

  describe('a failed motion POST stops the run and shows why (UI-14)', () => {
    const wavePosts = () =>
      vi.mocked(postTestingMitCommandBatch).mock.calls.filter(([batch]) =>
        batch.joints[0]?.name.startsWith('wave:'),
      ).length;
    const homePosts = () =>
      vi.mocked(postTestingMitCommandBatch).mock.calls.filter(([batch]) =>
        batch.joints.length > 0 && batch.joints.every((joint) => joint.position === 0),
      ).length;

    function expectStoppedWithError(
      result: { current: ReturnType<typeof useCompoundPlayback> },
      postsAtFailure: number,
    ) {
      expect(useCompoundStore.getState().isRunning).toBe(false);
      expect(result.current.overlayBlockReason).toMatch(/429 Testing rate limit/);
      // No return-home or any other command follows the failure.
      expect(vi.mocked(postTestingMitCommandBatch).mock.calls.length).toBe(postsAtFailure);
      expect(homePosts()).toBe(0);
      expect(useCompoundStore.getState().returnHomePending).toBe(false);
    }

    it('the first raise segment', async () => {
      useTestingStore.setState({ dryRun: false });
      vi.mocked(postTestingMitCommandBatch).mockRejectedValueOnce(
        new Error('testing mit command failed: 429 Testing rate limit'),
      );
      const { result } = renderPlayback();
      act(() => result.current.startRunner());
      await runFor(0);
      await runFor(20_000);
      expectStoppedWithError(result, 1);
    });

    it('a later raise segment', async () => {
      useTestingStore.setState({ dryRun: false });
      vi.mocked(postTestingMitCommandBatch)
        .mockResolvedValueOnce(undefined)
        .mockRejectedValueOnce(new Error('testing mit command failed: 429 Testing rate limit'));
      const { result } = renderPlayback();
      act(() => result.current.startRunner());
      await runFor(20_000);
      expect(wavePosts()).toBe(0);
      expectStoppedWithError(result, 2);
    });

    it('the first wave', async () => {
      useTestingStore.setState({ dryRun: false });
      vi.mocked(postTestingMitCommandBatch)
        .mockResolvedValueOnce(undefined)
        .mockResolvedValueOnce(undefined)
        .mockRejectedValueOnce(new Error('testing mit command failed: 429 Testing rate limit'));
      const { result } = renderPlayback();
      act(() => result.current.startRunner());
      await runFor(20_000);
      expect(wavePosts()).toBe(1);
      expectStoppedWithError(result, 3);
    });

    it('a looping wave', async () => {
      useTestingStore.setState({ dryRun: false });
      expect(useCompoundStore.getState().loop).toBe(true);
      vi.mocked(postTestingMitCommandBatch)
        .mockResolvedValueOnce(undefined)
        .mockResolvedValueOnce(undefined)
        .mockResolvedValueOnce(undefined)
        .mockRejectedValueOnce(new Error('testing mit command failed: 429 Testing rate limit'));
      const { result } = renderPlayback();
      act(() => result.current.startRunner());
      // Raise plus one 140 s wave, then the loop's next wave fails.
      await runFor(160_000);
      expect(wavePosts()).toBe(2);
      expectStoppedWithError(result, 4);
      // It stays stopped: no quiet retry a wave period later.
      await runFor(300_000);
      expect(vi.mocked(postTestingMitCommandBatch).mock.calls.length).toBe(4);
    });
  });

  it('a failed return-home POST is caught and shown, and the run still ends', async () => {
    useTestingStore.setState({ dryRun: false });
    vi.mocked(postTestingMitCommandBatch)
      .mockResolvedValueOnce(undefined)
      .mockRejectedValueOnce(new Error('testing mit command failed: 403 lease lost'));
    const { result } = renderPlayback();
    act(() => result.current.startRunner());
    await runFor(0);
    act(() => result.current.stopRunner());
    expect(useCompoundStore.getState().isRunning).toBe(false);
    await runFor(5_000);
    const calls = vi.mocked(postTestingMitCommandBatch).mock.calls;
    // Segment 0, then one return-home batch; no retry, no further command.
    expect(calls.length).toBe(2);
    expect(calls[1][0].joints.every((joint) => joint.position === 0)).toBe(true);
    expect(result.current.overlayBlockReason).toMatch(/^Return home failed: .*403 lease lost/);
    expect(useCompoundStore.getState().isRunning).toBe(false);
    expect(useCompoundStore.getState().returnHomePending).toBe(false);
  });
});

