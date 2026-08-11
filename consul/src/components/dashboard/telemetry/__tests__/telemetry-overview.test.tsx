// @vitest-environment jsdom
import '@testing-library/jest-dom/vitest';
import { cleanup, render, screen, waitFor, within } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { MemoryRouter } from 'react-router-dom';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';

import { TelemetryOverview } from '@/components/dashboard/telemetry/telemetry-overview';
import { TooltipProvider } from '@/components/ui/tooltip';
import { useRobotStore } from '@/state/robotStore';
import { useActuatorZeroStore } from '@/state/actuatorZeroStore';

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
    joints: ['right_shoulder_roll', 'right_shoulder_pitch'],
    motors: [
      {
        joint: 'right_shoulder_roll',
        can_interface: 'can0',
        device_id: 2,
        direction: 1,
        motor_type: 'rs03',
        bench: {
          position_lower_rad: -0.05,
          position_upper_rad: 2.5,
          torque_limit_nm: 5,
        },
      },
      {
        joint: 'right_shoulder_pitch',
        can_interface: 'can0',
        device_id: 1,
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

vi.mock('@/lib/chappe-config', () => ({
  isChappeLive: () => true,
}));

function renderTelemetry() {
  const client = new QueryClient({
    defaultOptions: { queries: { retry: false } },
  });
  return render(
    <QueryClientProvider client={client}>
      <MemoryRouter>
        <TooltipProvider>
          <TelemetryOverview />
        </TooltipProvider>
      </MemoryRouter>
    </QueryClientProvider>,
  );
}

afterEach(() => {
  cleanup();
  useRobotStore.setState({
    connected: false,
    robotState: null,
    operationalMode: null,
  });
  useActuatorZeroStore.setState({ zeroed: {} });
  vi.clearAllMocks();
});

describe('TelemetryOverview', () => {
  it('renders the live master inventory table shell', async () => {
    renderTelemetry();
    expect(await screen.findByTestId('telemetry-overview')).toBeTruthy();
    expect(await screen.findByTestId('inventory-table-shell')).toBeTruthy();
  });

  it('shows unknown Reference facet when wire homing_state is absent', async () => {
    useActuatorZeroStore.getState().markZeroed('right_shoulder_pitch');
    useRobotStore.setState({
      connected: true,
      robotState: {
        $typeName: 'marengo.v1.RobotState',
        timestamp: { $typeName: 'google.protobuf.Timestamp', seconds: 0n, nanos: 0 },
        joints: [
          {
            $typeName: 'marengo.v1.JointState',
            name: 'right_shoulder_pitch',
            position: 1.2,
            velocity: 0,
            effort: 0,
            temperatureC: 30,
            fault: 0,
          },
        ],
      } as never,
    });

    renderTelemetry();
    const overview = await screen.findByTestId('telemetry-overview');
    await waitFor(() => {
      expect(within(overview).getAllByText(/unknown|n\/a|not available/i).length).toBeGreaterThan(
        0,
      );
    });
    expect(within(overview).queryByText(/^Ready$/i)).toBeNull();
  });
});
