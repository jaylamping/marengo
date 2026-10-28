// @vitest-environment jsdom
import { act, renderHook } from '@testing-library/react';
import { afterEach, beforeEach, expect, it, vi } from 'vitest';

import { usePostEnableWatch } from '@/hooks/use-post-enable-watch';
import { useRobotStore } from '@/state/robotStore';

beforeEach(() => {
  vi.useFakeTimers();
  vi.setSystemTime(new Date(1_000_000));
  useRobotStore.setState({
    operationalMode: 'DISABLED',
    safetyState: null,
    robotState: null,
    lastActionRefusal: null,
  });
});

afterEach(() => {
  vi.useRealTimers();
  useRobotStore.setState({ operationalMode: null, lastActionRefusal: null });
});

function startWatch() {
  const watch = renderHook(() => usePostEnableWatch(null, null));
  act(() => watch.result.current.start());
  return watch;
}

it('a slow Enable that goes ACTIVE after 2 s is not reported as tripped', () => {
  const watch = startWatch();
  // Gravity preflight, stagger and a post-SetZero quiet: still DISABLED at 2 s.
  act(() => vi.advanceTimersByTime(2000));
  expect(watch.result.current.outcome?.kind).toBe('pending');
  act(() => useRobotStore.setState({ operationalMode: 'ACTIVE' }));
  act(() => vi.advanceTimersByTime(600));
  expect(watch.result.current.outcome?.kind).toBe('pending');
  act(() => vi.advanceTimersByTime(1000));
  expect(watch.result.current.outcome).toEqual({ kind: 'ok', message: 'Motors ACTIVE.' });
  watch.unmount();
});

it('ACTIVE then DISABLED is reported as a trip', () => {
  const watch = startWatch();
  act(() => vi.advanceTimersByTime(400));
  act(() => useRobotStore.setState({ operationalMode: 'ACTIVE' }));
  act(() => vi.advanceTimersByTime(400));
  act(() => useRobotStore.setState({ operationalMode: 'DISABLED' }));
  act(() => vi.advanceTimersByTime(200));
  expect(watch.result.current.outcome?.kind).toBe('error');
  expect(watch.result.current.outcome?.message).toMatch(/returned to DISABLED/);
  watch.unmount();
});

it('a published enable refusal ends the watch with the Pi reason', () => {
  const watch = startWatch();
  act(() =>
    useRobotStore.setState({
      lastActionRefusal: {
        receivedAtMs: Date.now(),
        action: 'motion_refused',
        reason: 'enable refused: motion is owned by stdin, not chappe',
      },
    }),
  );
  act(() => vi.advanceTimersByTime(200));
  expect(watch.result.current.outcome?.kind).toBe('error');
  expect(watch.result.current.outcome?.message).toMatch(/owned by stdin/);
  watch.unmount();
});

it('an older or unrelated refusal does not fail the Enable', () => {
  useRobotStore.setState({
    lastActionRefusal: {
      receivedAtMs: Date.now() - 60_000,
      action: 'motion_refused',
      reason: 'enable refused: an old one',
    },
  });
  const watch = startWatch();
  act(() =>
    useRobotStore.setState({
      lastActionRefusal: {
        receivedAtMs: Date.now(),
        action: 'motion_refused',
        reason: 'testing batch refused: motion is owned by stdin',
      },
    }),
  );
  act(() => vi.advanceTimersByTime(1000));
  expect(watch.result.current.outcome?.kind).toBe('pending');
  watch.unmount();
});

it('an Enable that never reaches ACTIVE fails only after the Pi bound', () => {
  const watch = startWatch();
  act(() => vi.advanceTimersByTime(3000));
  expect(watch.result.current.outcome?.kind).toBe('pending');
  act(() => vi.advanceTimersByTime(1200));
  expect(watch.result.current.outcome?.kind).toBe('error');
  expect(watch.result.current.outcome?.message).toMatch(/did not reach ACTIVE/);
  watch.unmount();
});
