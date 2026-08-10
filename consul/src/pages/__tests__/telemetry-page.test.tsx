// @vitest-environment jsdom
import '@testing-library/jest-dom/vitest';
import { cleanup, fireEvent, render, screen, waitFor, within } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { MemoryRouter, Route, Routes, useLocation } from 'react-router-dom';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';

import { TelemetryPage } from '@/pages/telemetry';
import { SubsystemsPage } from '@/pages/subsystems';
import { TooltipProvider } from '@/components/ui/tooltip';
import { InventoryDetailProvider } from '@/components/dashboard/inventory/inventory-detail-context';
import { InventoryRowModal } from '@/components/dashboard/inventory/inventory-row-modal';
import type { InventoryItem } from '@/data/robot-inventory';

vi.mock('@/hooks/use-mobile', () => ({
  useIsMobile: () => false,
}));

vi.mock('@/hooks/use-active-reporting-lease', () => ({
  useActiveReportingLease: () => 'idle',
}));

vi.mock('@/lib/config-api', () => ({
  fetchConfigSnapshot: vi.fn(async () => ({
    profile: 'master',
    config_dir: '/opt/marengo/config',
    joints: ['right_shoulder_pitch'],
    motors: [
      {
        joint: 'right_shoulder_pitch',
        can_interface: 'can0',
        device_id: 2,
        direction: -1,
        motor_type: 'rs03',
        bench: {
          position_lower_rad: -0.9,
          position_upper_rad: 2.9,
          torque_limit_nm: 5,
        },
      },
    ],
    control_limits: [],
  })),
}));

const actuator: InventoryItem = {
  id: 27,
  name: 'right_shoulder_pitch',
  group: 'right_arm',
  kind: 'actuator',
  status: 'Enabled',
  value: '1.20',
  limit: '−0.90–2.90',
  preset: 'unassigned',
  node: 'RS03 · can0 · id 2',
};

function LocationProbe() {
  const location = useLocation();
  return <div data-testid="location">{location.pathname}</div>;
}

function renderPage() {
  const client = new QueryClient({
    defaultOptions: { queries: { retry: false } },
  });
  return render(
    <QueryClientProvider client={client}>
      <MemoryRouter initialEntries={['/telemetry']}>
        <TooltipProvider>
          <Routes>
            <Route path="/telemetry" element={<TelemetryPage />} />
            <Route path="/hardware" element={<div>Hardware</div>} />
          </Routes>
        </TooltipProvider>
      </MemoryRouter>
    </QueryClientProvider>,
  );
}

afterEach(() => {
  cleanup();
  vi.clearAllMocks();
});

describe('TelemetryPage', () => {
  it('renders telemetry overview (not the Phase 1 stub)', async () => {
    renderPage();
    expect(screen.queryByTestId('telemetry-stub')).toBeNull();
    expect(await screen.findByTestId('telemetry-overview')).toBeTruthy();
  });
});

describe('/subsystems redirect into Telemetry', () => {
  it('lands on /telemetry without Inventory commissioning chrome', () => {
    render(
      <MemoryRouter initialEntries={['/subsystems']}>
        <Routes>
          <Route path="/subsystems" element={<SubsystemsPage />} />
          <Route
            path="/telemetry"
            element={
              <>
                <LocationProbe />
                <div data-testid="telemetry-landed">Telemetry</div>
              </>
            }
          />
        </Routes>
      </MemoryRouter>,
    );
    expect(screen.getByTestId('location').textContent).toBe('/telemetry');
    expect(screen.getByTestId('telemetry-landed')).toBeTruthy();
    expect(screen.queryByRole('button', { name: 'Set Limits' })).toBeNull();
    expect(screen.queryByRole('button', { name: 'Enable' })).toBeNull();
    expect(screen.queryByRole('button', { name: 'Home' })).toBeNull();
  });
});

describe('Telemetry / Inventory modal — no commissioning actions', () => {
  it('shows read-only limits with Hardware redirect and no Set Limits/Home/go-to-zero', async () => {
    render(
      <MemoryRouter>
        <TooltipProvider>
          <InventoryDetailProvider openItem={() => undefined}>
            <InventoryRowModal
              item={actuator}
              items={[actuator]}
              open
              onOpenChange={() => undefined}
              onNavigate={() => undefined}
            />
          </InventoryDetailProvider>
        </TooltipProvider>
      </MemoryRouter>,
    );

    const modal = await screen.findByTestId('inventory-row-modal');
    expect(within(modal).getByTestId('inventory-limits-readonly')).toBeTruthy();
    expect(within(modal).getByText(/Range \(live\):/)).toBeTruthy();
    const link = within(modal).getByRole('link', { name: /Calibrate on Hardware/i });
    expect(link.getAttribute('href')).toBe('/hardware');

    expect(within(modal).queryByRole('button', { name: 'Set Limits' })).toBeNull();
    expect(within(modal).queryByRole('button', { name: 'Apply Limits' })).toBeNull();
    expect(within(modal).queryByRole('button', { name: 'Home' })).toBeNull();
    expect(within(modal).queryByRole('button', { name: 'Go' })).toBeNull();
    expect(within(modal).queryByRole('button', { name: 'Start sweep' })).toBeNull();
    expect(within(modal).queryByRole('button', { name: /Enable/i })).toBeNull();
    expect(within(modal).queryByRole('button', { name: /go to zero/i })).toBeNull();

    // Reference facet placeholder when wire fields are absent
    expect(within(modal).getByTestId('telemetry-reference-facet')).toHaveTextContent(
      /unknown|n\/a|not available/i,
    );
  });
});

describe('Row actions menu — no commissioning', () => {
  it('does not offer Zero/home or Disable commissioning items', async () => {
    const { RowActionsMenu } = await import(
      '@/components/dashboard/inventory/cells/row-actions-menu'
    );
    render(
      <MemoryRouter>
        <TooltipProvider>
          <RowActionsMenu />
        </TooltipProvider>
      </MemoryRouter>,
    );
    const trigger = screen.getByRole('button', { name: /Open menu/i });
    fireEvent.pointerDown(trigger, { button: 0, ctrl: 0 });
    fireEvent.click(trigger);
    await waitFor(() => {
      expect(screen.queryByText(/Zero\s*\/\s*home/i)).toBeNull();
      expect(screen.queryByText(/^Disable$/i)).toBeNull();
      expect(screen.queryByText(/^Enable$/i)).toBeNull();
      expect(screen.getByText(/Open on Hardware/i)).toBeTruthy();
    });
  });
});
