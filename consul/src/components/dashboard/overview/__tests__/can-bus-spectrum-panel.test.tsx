// @vitest-environment jsdom
import { afterEach, describe, expect, it, vi } from 'vitest';
import { cleanup, render, screen } from '@testing-library/react';
import { MemoryRouter } from 'react-router-dom';

import { CanBusSpectrumPanel } from '@/components/dashboard/overview/can-bus-spectrum-panel';
import type { CanTrafficSpectrumView } from '@/hooks/use-can-traffic-spectrum';

const emptySpectrum: CanTrafficSpectrumView = {
  source: 'empty',
  presence: 'absent',
  fingerprint: null,
  capturedAtMs: 0,
  durationS: 0,
  parsedFrames: 0,
  sessionApproxHz: null,
  bands: [],
  partitions: [],
  rateHz: [],
  microLog: [],
  live: {
    iface: 'can0',
    canState: 'ERROR-ACTIVE',
    warn: false,
    rxBytesPerSec: null,
    txBytesPerSec: null,
    txErrorCount: null,
    rxErrorCount: null,
  },
  errorKind: null,
  logsCanHref: '/logs',
  loading: false,
};

const mockView = vi.fn((): CanTrafficSpectrumView => emptySpectrum);

vi.mock('@/hooks/use-can-traffic-spectrum', () => ({
  useCanTrafficSpectrum: () => mockView(),
}));

afterEach(() => {
  cleanup();
  mockView.mockReset();
  mockView.mockImplementation(() => emptySpectrum);
});

describe('CanBusSpectrumPanel', () => {
  it('renders the overview CAN panel with empty-capture copy', () => {
    render(
      <MemoryRouter>
        <CanBusSpectrumPanel />
      </MemoryRouter>,
    );
    expect(screen.getByTestId('overview-can-bus-panel')).toBeTruthy();
    expect(screen.getByText('CAN bus')).toBeTruthy();
    expect(screen.getByText(/No harness candump yet/i)).toBeTruthy();
    expect(screen.getByTestId('can-live-chip').textContent).toMatch(/can0/);
    expect(screen.getByRole('link', { name: /Open Logs/i }).getAttribute('href')).toBe(
      '/logs',
    );
  });

  it('shows a loading capture state before the first poll settles', () => {
    mockView.mockImplementation(() => ({
      ...emptySpectrum,
      loading: true,
    }));
    render(
      <MemoryRouter>
        <CanBusSpectrumPanel />
      </MemoryRouter>,
    );
    expect(screen.getByText(/Loading capture/i)).toBeTruthy();
  });

  it('does not render zeroed spectrum chrome when capture is unavailable', () => {
    mockView.mockImplementation(() => ({
      ...emptySpectrum,
      source: 'unavailable',
      loading: false,
      errorKind: { kind: 'no_endpoint' },
    }));
    render(
      <MemoryRouter>
        <CanBusSpectrumPanel />
      </MemoryRouter>,
    );
    expect(screen.getByText(/Gateway offline/i)).toBeTruthy();
    expect(screen.queryByText('Top IDs')).toBeNull();
    expect(screen.queryByText('Rate')).toBeNull();
  });
});
