import { act, renderHook } from '@testing-library/react';
import { create, toBinary } from '@bufbuild/protobuf';
import { afterEach, expect, it, vi } from 'vitest';
import { EnvelopeSchema, RobotStateSchema, SafetyStateSchema, RuntimeConnectionStateSchema, OperationalMode } from '@/gen/marengo/v1/marengo_pb';
import type { ChappeTelemetryHandlers } from '@/lib/chappe-transport';
import { dispatchEnvelope } from '@/lib/chappe-transport';
import { useChappeTelemetry } from '@/hooks/use-chappe-telemetry';
import { useRobotStore } from '@/state/robotStore';
import { useHostMetricsStore } from '@/state/hostMetricsStore';

let handlers: ChappeTelemetryHandlers;
vi.mock('@/lib/chappe-client', () => ({ connectChappeStream: vi.fn(async (next: ChappeTelemetryHandlers) => { handlers = next; return () => {}; }) }));
vi.mock('@/lib/chappe-config', () => ({ isChappeLive: () => true, getChappeEndpoints: () => null, getChappeSubscribeTopics: () => [] }));
vi.mock('@/lib/log-buffer', () => ({ enableChappeLiveLogs: () => {}, appendLiveLog: () => {}, shouldDecodeLogEvents: () => false }));
afterEach(() => { vi.useRealTimers(); });

it('typed IPC transitions retire live facts and cancel queued old telemetry while the gateway stays open', async () => {
  vi.useFakeTimers();
  vi.setSystemTime(new Date(100000));
  const mounted = renderHook(() => useChappeTelemetry());
  await act(async () => {});
  const state = (timestampMs: bigint) => create(RobotStateSchema, { timestampMs, joints: [] });
  act(() => {
    handlers.onConnected?.();
    handlers.onRobotState(state(10n));
    handlers.onSafetyState(create(SafetyStateSchema, { mode: OperationalMode.ACTIVE }));
    handlers.onRobotState(state(20n)); // pending trailing update from old peer
  });
  expect(useRobotStore.getState().robotState?.timestampMs).toBe(10n);
  const transition = (generation: bigint, connected: boolean) => toBinary(EnvelopeSchema, create(EnvelopeSchema, {
    sourceNode: 'marengo-gateway', messageType: 'marengo.v1.RuntimeConnectionState',
    payload: toBinary(RuntimeConnectionStateSchema, create(RuntimeConnectionStateSchema, { generation, connected })),
  }));
  act(() => dispatchEnvelope(transition(2n, false), handlers));
  expect(useRobotStore.getState().robotState).toBeNull();
  expect(useRobotStore.getState().safetyState).toBeNull();
  expect(useRobotStore.getState().operationalMode).toBeNull();
  expect(useRobotStore.getState().connected).toBe(false);
  expect(useHostMetricsStore.getState().piMetrics).toBeNull();
  act(() => vi.advanceTimersByTime(1000));
  expect(useRobotStore.getState().robotState).toBeNull();
  act(() => dispatchEnvelope(transition(3n, true), handlers));
  expect(useRobotStore.getState().connected).toBe(false); // socket is not producer evidence
  act(() => handlers.onRobotState(state(30n)));
  expect(useRobotStore.getState().robotState?.timestampMs).toBe(30n);
  mounted.unmount();
});
