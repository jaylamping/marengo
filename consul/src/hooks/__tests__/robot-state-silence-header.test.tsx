// @vitest-environment jsdom
import '@testing-library/jest-dom/vitest';
import { create } from '@bufbuild/protobuf';
import { act, cleanup, render, renderHook, screen } from '@testing-library/react';
import { afterEach, beforeEach, expect, it, vi } from 'vitest';

import { SiteHeaderStatusBadges } from '@/components/dashboard/site-header/site-header-status-badges';
import { TooltipProvider } from '@/components/ui/tooltip';
import {
  OperationalMode,
  RobotStateSchema,
  SafetyStateSchema,
} from '@/gen/marengo/v1/marengo_pb';
import type { ChappeTelemetryHandlers } from '@/lib/chappe-transport';
import { ROBOT_STATE_STALE_MS, useChappeTelemetry } from '@/hooks/use-chappe-telemetry';
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
  resolveChappeEndpoints: () => ({
    endpoints: { webTransportUrl: 'https://bench:4433', httpUrl: 'https://bench:8443' },
    source: 'baked',
  }),
  chappeConnectionErrDetail: () => null,
  chappeMisconfigHint: () => null,
}));
vi.mock('@/lib/log-buffer', () => ({
  enableChappeLiveLogs: () => {},
  appendLiveLog: () => {},
  shouldDecodeLogEvents: () => false,
}));

function machineLabel() {
  return screen.getByTestId('machine-state').textContent;
}

async function mountTelemetryAndHeader() {
  const hook = renderHook(() => useChappeTelemetry());
  await act(async () => {});
  render(
    <TooltipProvider>
      <SiteHeaderStatusBadges />
    </TooltipProvider>,
  );
  return hook;
}

beforeEach(() => {
  vi.useFakeTimers();
});

afterEach(() => {
  cleanup();
  vi.useRealTimers();
  useRobotStore.setState({
    connected: false,
    robotStateStale: false,
    safetyState: null,
    operationalMode: null,
    robotState: null,
    gatewayError: null,
  });
});

it('RobotState silence shows RUNTIME SILENT, not CONNECTING, and keeps SafetyState', async () => {
  const hook = await mountTelemetryAndHeader();
  act(() => {
    handlers.onConnected?.();
    handlers.onSafetyState(create(SafetyStateSchema, { mode: OperationalMode.ACTIVE }));
    handlers.onRobotState(create(RobotStateSchema, { timestampMs: 10n }));
  });
  expect(machineLabel()).toBe('ACTIVE');

  act(() => vi.advanceTimersByTime(ROBOT_STATE_STALE_MS + 100));
  const state = useRobotStore.getState();
  expect(state.connected).toBe(false);
  expect(state.robotStateStale).toBe(true);
  expect(state.robotState).toBeNull();
  expect(state.operationalMode).toBeNull();
  expect(state.safetyState).not.toBeNull();
  expect(machineLabel()).toBe('RUNTIME SILENT');
  expect(machineLabel()).not.toBe('CONNECTING');

  // Fresh RobotState clears the stale flag; the next SafetyState restores the mode.
  act(() => {
    handlers.onRobotState(create(RobotStateSchema, { timestampMs: 20n }));
    handlers.onSafetyState(create(SafetyStateSchema, { mode: OperationalMode.ACTIVE }));
  });
  expect(useRobotStore.getState().robotStateStale).toBe(false);
  expect(machineLabel()).toBe('ACTIVE');
  hook.unmount();
});

it('a latched fault or E-STOP published during RobotState silence outranks RUNTIME SILENT', async () => {
  const hook = await mountTelemetryAndHeader();
  act(() => {
    handlers.onConnected?.();
    handlers.onSafetyState(create(SafetyStateSchema, { mode: OperationalMode.ACTIVE }));
    handlers.onRobotState(create(RobotStateSchema, { timestampMs: 10n }));
  });
  act(() => vi.advanceTimersByTime(ROBOT_STATE_STALE_MS + 100));
  expect(machineLabel()).toBe('RUNTIME SILENT');

  // RobotState stays silent, SafetyState keeps arriving and now reports a latch.
  act(() =>
    handlers.onSafetyState(
      create(SafetyStateSchema, {
        mode: OperationalMode.DISABLED,
        softwareEstopLatched: true,
      }),
    ),
  );
  expect(machineLabel()).toBe('FAULT LATCHED');

  act(() =>
    handlers.onSafetyState(
      create(SafetyStateSchema, {
        mode: OperationalMode.DISABLED,
        softwareEstopLatched: true,
        hardwareEstopAsserted: true,
      }),
    ),
  );
  expect(machineLabel()).toBe('E-STOP');
  hook.unmount();
});

it('a fresh SafetyState E-STOP shows before the first RobotState arrives', async () => {
  const hook = await mountTelemetryAndHeader();
  act(() => handlers.onConnected?.());
  expect(machineLabel()).toBe('CONNECTING');
  act(() =>
    handlers.onSafetyState(
      create(SafetyStateSchema, {
        mode: OperationalMode.DISABLED,
        hardwareEstopAsserted: true,
      }),
    ),
  );
  expect(machineLabel()).toBe('E-STOP');
  hook.unmount();
});

it('a connection-level retirement clears the stale flag back to CONNECTING', async () => {
  const hook = await mountTelemetryAndHeader();
  act(() => {
    handlers.onConnected?.();
    handlers.onRobotState(create(RobotStateSchema, { timestampMs: 10n }));
  });
  act(() => vi.advanceTimersByTime(ROBOT_STATE_STALE_MS + 100));
  expect(machineLabel()).toBe('RUNTIME SILENT');
  act(() => handlers.onDisconnected?.());
  expect(useRobotStore.getState().robotStateStale).toBe(false);
  expect(machineLabel()).toBe('CONNECTING');
  hook.unmount();
});
