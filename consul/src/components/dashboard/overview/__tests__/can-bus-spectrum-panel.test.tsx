// @vitest-environment jsdom
import { afterEach, describe, expect, it, vi } from 'vitest';
import { cleanup, render, screen } from '@testing-library/react';
import { MemoryRouter } from 'react-router-dom';

import { CanBusSpectrumPanel } from '@/components/dashboard/overview/can-bus-spectrum-panel';
import type { CanTrafficSpectrum } from '@/lib/can-traffic-spectrum';

const emptySpectrum: CanTrafficSpectrum = {
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
};

vi.mock('@/hooks/use-can-traffic-spectrum', () => ({
  useCanTrafficSpectrum: vi.fn(() => emptySpectrum),
}));

afterEach(() => {
  cleanup();
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
    expect(screen.getAllByText(/No harness candump yet/i).length).toBeGreaterThan(0);
    expect(screen.getByTestId('can-live-chip').textContent).toMatch(/can0/);
    expect(screen.getByRole('link', { name: /Open Logs/i }).getAttribute('href')).toBe(
      '/logs',
    );
  });
});
