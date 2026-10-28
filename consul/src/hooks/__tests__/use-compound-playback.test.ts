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
});
