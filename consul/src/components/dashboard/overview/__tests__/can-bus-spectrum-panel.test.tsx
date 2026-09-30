// @vitest-environment jsdom
import { afterEach, describe, expect, it, vi } from 'vitest';
import { cleanup, render, screen } from '@testing-library/react';
import { MemoryRouter } from 'react-router-dom';

import { CanBusSpectrumPanel } from '@/components/dashboard/overview/can-bus-spectrum-panel';
import type { CanTrafficSpectrumView } from '@/hooks/use-can-traffic-spectrum';

const emptyView: CanTrafficSpectrumView = {
  capture: {
    status: 'empty',
    live: {
      iface: 'can0',
      canState: 'ERROR-ACTIVE',
      warn: false,
      rxBytesPerSec: null,
      txBytesPerSec: null,
      txErrorCount: null,
      rxErrorCount: null,
    },
  },
  loading: false,
  linkActivity: [
    { atMs: 1_000, rxBps: 0, txBps: 0 },
    { atMs: 2_000, rxBps: 12, txBps: 4 },
  ],
};

const mockView = vi.fn((): CanTrafficSpectrumView => emptyView);

vi.mock('@/hooks/use-can-traffic-spectrum', () => ({
  useCanTrafficSpectrum: () => mockView(),
}));

afterEach(() => {
  cleanup();
  mockView.mockReset();
  mockView.mockImplementation(() => emptyView);
});

describe('CanBusSpectrumPanel', () => {
  it('renders the overview CAN panel with empty-capture copy and link activity graph', () => {
    render(
      <MemoryRouter>
        <CanBusSpectrumPanel />
      </MemoryRouter>,
    );
    expect(screen.getByTestId('overview-can-bus-panel')).toBeTruthy();
    expect(screen.getByText('CAN bus')).toBeTruthy();
    expect(screen.getByText(/No harness candump/i)).toBeTruthy();
    expect(screen.getByTestId('can-live-chip').textContent).toMatch(/can0/);
    expect(screen.getByTestId('can-link-activity')).toBeTruthy();
    expect(screen.getByTestId('can-capture-status')).toBeTruthy();
    expect(screen.getByText(/^Link$/i)).toBeTruthy();
    expect(screen.getByRole('link', { name: /Open Logs/i }).getAttribute('href')).toBe(
      '/logs',
    );
  });

  it('shows a loading capture state before the first poll settles', () => {
    mockView.mockImplementation(() => ({
      ...emptyView,
      loading: true,
    }));
    render(
      <MemoryRouter>
        <CanBusSpectrumPanel />
      </MemoryRouter>,
    );
    expect(screen.getByText(/Loading capture/i)).toBeTruthy();
    expect(screen.getByTestId('can-link-activity')).toBeTruthy();
  });

  it('does not render zeroed spectrum chrome when capture is unavailable', () => {
    mockView.mockImplementation(() => ({
      ...emptyView,
      loading: false,
      capture: {
        status: 'unavailable',
        live: emptyView.capture.live,
        error: { kind: 'no_endpoint' },
      },
    }));
    render(
      <MemoryRouter>
        <CanBusSpectrumPanel />
      </MemoryRouter>,
    );
    expect(screen.getByText(/No gateway/i)).toBeTruthy();
    expect(screen.getByText(/Wireframe/i)).toBeTruthy();
    expect(screen.queryByText('Top IDs')).toBeNull();
    expect(screen.queryByText('Rate')).toBeNull();
    expect(screen.getByTestId('can-link-activity')).toBeTruthy();
  });
});
