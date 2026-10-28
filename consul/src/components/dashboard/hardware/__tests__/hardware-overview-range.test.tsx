// @vitest-environment jsdom
import '@testing-library/jest-dom/vitest';
import { create } from '@bufbuild/protobuf';
import { act, cleanup, fireEvent, render, screen, waitFor } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { MemoryRouter } from 'react-router-dom';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';

import { HardwareOverview } from '@/components/dashboard/hardware/hardware-overview';
import { TooltipProvider } from '@/components/ui/tooltip';
import {
  ActuatorLimitSnapshotSchema,
  JointActuatorLimitSchema,
} from '@/gen/marengo/v1/marengo_pb';
import { useActuatorStore } from '@/state/actuatorStore';

// Stand-in for the Set Limits flow: Apply reports a display range upward.
vi.mock('@/components/dashboard/inventory/set-limits-panel', () => ({
  SetLimitsPanel: ({ onApplyRange }: { onApplyRange?: (range: string) => void }) => (
    <button type="button" onClick={() => onApplyRange?.('0.00–1.00')}>
      Fake Apply
    </button>
  ),
}));

vi.mock('@/lib/hardware-api', () => ({
  fetchCompleteness: vi.fn(async () => ({ warnings: [] })),
  uploadUrdf: vi.fn(),
  resolveUrdfPreview: vi.fn(),
  activateUrdf: vi.fn(),
  fetchUrdfArchiveList: vi.fn(async () => ({ entries: [] })),
  restoreUrdfArchive: vi.fn(),
}));

vi.mock('@/lib/config-api', () => ({
  fetchConfigSnapshot: vi.fn(async () => ({
    profile: 'master',
    config_dir: '/opt/marengo/config',
    joints: ['right_shoulder_roll'],
    motors: [
      {
        joint: 'right_shoulder_roll',
        can_interface: 'can0',
        device_id: 2,
        direction: 1,
        motor_type: 'rs03',
        bench: { position_lower_rad: -0.05, position_upper_rad: 2.5, torque_limit_nm: 5 },
      },
    ],
    control_limits: [],
  })),
}));

vi.mock('@/lib/gateway-api', () => ({
  postSetZeroCommand: vi.fn(),
  postEnableCommand: vi.fn(async () => undefined),
  postActiveReportingLease: vi.fn(async () => undefined),
  postMotorStatusPoll: vi.fn(async () => undefined),
  fetchActuatorLimits: vi.fn(async () => null),
  fetchCommissioningScope: vi.fn(async () => ({
    version: 1,
    joints: [],
    ceiling: null,
    effective: [],
    persisted: false,
  })),
  putCommissioningScope: vi.fn(),
  deleteCommissioningScope: vi.fn(),
}));

beforeEach(() => {
  class ResizeObserverMock {
    observe() {}
    unobserve() {}
    disconnect() {}
  }
  vi.stubGlobal('ResizeObserver', ResizeObserverMock);
});

afterEach(() => {
  cleanup();
  useActuatorStore.getState().setLimitSnapshot(null);
  vi.clearAllMocks();
});

describe('HardwareOverview range (UI-17)', () => {
  it('after Apply, a later live snapshot range is shown, not the applied string', async () => {
    const client = new QueryClient({ defaultOptions: { queries: { retry: false } } });
    render(
      <QueryClientProvider client={client}>
        <MemoryRouter>
          <TooltipProvider>
            <HardwareOverview />
          </TooltipProvider>
        </MemoryRouter>
      </QueryClientProvider>,
    );
    fireEvent.click(await screen.findByTestId('hardware-row-right_shoulder_roll'));
    fireEvent.click(await screen.findByRole('button', { name: 'Fake Apply' }));

    // Another tab, MCP or the server clamps the limit: the live snapshot moves.
    act(() => {
      useActuatorStore.getState().setLimitSnapshot(
        create(ActuatorLimitSnapshotSchema, {
          joints: [
            create(JointActuatorLimitSchema, {
              joint: 'right_shoulder_roll',
              posLowerRad: -0.1,
              posUpperRad: 1.5,
            }),
          ],
        }),
      );
    });

    await waitFor(() => {
      expect(screen.getByTestId('hardware-row-right_shoulder_roll')).toHaveTextContent(
        '-0.10–1.50',
      );
    });
  });
});
