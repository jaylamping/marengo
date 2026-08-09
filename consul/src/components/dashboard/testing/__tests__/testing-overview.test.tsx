// @vitest-environment jsdom
import '@testing-library/jest-dom/vitest';
import { cleanup, render, screen, waitFor } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';

import { TestingOverview } from '@/components/dashboard/testing/testing-overview';
import { useRobotStore } from '@/state/robotStore';
import { useTestingStore } from '@/state/testingStore';

vi.mock('@/hooks/use-config-snapshot', () => ({
  useConfigSnapshot: () => ({ data: null }),
}));

vi.mock('@/lib/gateway-api', () => ({
  postEnableCommand: vi.fn(),
  postHomeCommand: vi.fn(),
  postTestingMitCommandBatch: vi.fn(),
}));

function renderTesting() {
  const client = new QueryClient({
    defaultOptions: { queries: { retry: false } },
  });
  return render(
    <QueryClientProvider client={client}>
      <TestingOverview />
    </QueryClientProvider>,
  );
}

afterEach(() => {
  cleanup();
  useRobotStore.setState({
    connected: false,
    operationalMode: null,
    robotState: null,
  });
  useTestingStore.setState({
    selectedJointNames: [],
    isRunning: false,
    dryRun: true,
    setpointRad: 0,
  });
  vi.clearAllMocks();
});

beforeEach(() => {
  class ResizeObserverMock {
    observe() {}
    unobserve() {}
    disconnect() {}
  }
  vi.stubGlobal('ResizeObserver', ResizeObserverMock);
  vi.stubGlobal(
    'requestIdleCallback',
    (cb: IdleRequestCallback) => {
      cb({ didTimeout: false, timeRemaining: () => 50 } as IdleDeadline);
      return 1;
    },
  );
  vi.stubGlobal('cancelIdleCallback', () => {});
});

describe('TestingOverview — commissioning cutover', () => {
  it('does not offer Enable or Home/Mark READY', async () => {
    renderTesting();
    await waitFor(() => {
      expect(screen.getByTestId('testing-overview')).toBeTruthy();
    });
    expect(screen.queryByRole('button', { name: /^Enable$/i })).toBeNull();
    expect(screen.queryByRole('button', { name: /^Home$/i })).toBeNull();
    expect(screen.queryByRole('button', { name: /Homing/i })).toBeNull();
    expect(screen.queryByRole('button', { name: /^Disable$/i })).toBeNull();
  });

  it('keeps motion hold controls and E-stop', async () => {
    renderTesting();
    await waitFor(() => {
      expect(screen.getByTestId('testing-overview')).toBeTruthy();
    });
    expect(screen.getByRole('button', { name: /E-STOP/i })).toBeTruthy();
    expect(screen.getByRole('button', { name: /Start Hold/i })).toBeTruthy();
  });

  it('exposes go-to-zero (Return Home motion) when a hold is running', async () => {
    useTestingStore.setState({ isRunning: true });
    renderTesting();
    await waitFor(() => {
      expect(screen.getByRole('button', { name: /Return Home/i })).toBeTruthy();
    });
    expect(screen.queryByRole('button', { name: /^Home$/i })).toBeNull();
  });
});
