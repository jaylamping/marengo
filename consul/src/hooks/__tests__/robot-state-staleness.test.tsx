import { act, renderHook } from '@testing-library/react';
import { create } from '@bufbuild/protobuf';
import { afterEach, expect, it, vi } from 'vitest';

import { OperationalMode, RobotStateSchema, SafetyStateSchema } from '@/gen/marengo/v1/marengo_pb';
import type { ChappeTelemetryHandlers } from '@/lib/chappe-transport';
import { useChappeTelemetry } from '@/hooks/use-chappe-telemetry';
import { useRobotStore } from '@/state/robotStore';

let handlers: ChappeTelemetryHandlers;
vi.mock('@/lib/chappe-client', () => ({
  connectChappeStream: vi.fn(async (next: ChappeTelemetryHandlers) => {
    handlers = next;
    return () => {};
  }),
}));
vi.mock('@/lib/chappe-config', () => ({
  isChappeLive: () => true,
  getChappeEndpoints: () => null,
  getChappeSubscribeTopics: () => [],
}));
vi.mock('@/lib/log-buffer', () => ({
  enableChappeLiveLogs: () => {},
  appendLiveLog: () => {},
  shouldDecodeLogEvents: () => false,
}));
afterEach(() => {
  vi.useRealTimers();
});

it('a silent RobotState stream retires the runtime facts as stale (UI-11)', async () => {
  vi.useFakeTimers();
  const mounted = renderHook(() => useChappeTelemetry());
  await act(async () => {});
  act(() => {
    handlers.onSafetyState(create(SafetyStateSchema, { mode: OperationalMode.ACTIVE }));
    handlers.onRobotState(create(RobotStateSchema, { timestampMs: 10n }));
  });
  act(() => vi.advanceTimersByTime(1000));
  expect(useRobotStore.getState().connected).toBe(true);
  expect(useRobotStore.getState().operationalMode).toBe('ACTIVE');

  // The loop stalls with the gateway and IPC still up: no more RobotState.
  act(() => vi.advanceTimersByTime(1000));
  expect(useRobotStore.getState().connected).toBe(false);
  expect(useRobotStore.getState().operationalMode).toBeNull();
  expect(useRobotStore.getState().robotState).toBeNull();

  // Fresh state brings it back.
  act(() => handlers.onRobotState(create(RobotStateSchema, { timestampMs: 20n })));
  expect(useRobotStore.getState().connected).toBe(true);
  mounted.unmount();
});
