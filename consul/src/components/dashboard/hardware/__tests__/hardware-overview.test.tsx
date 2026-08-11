// @vitest-environment jsdom
import '@testing-library/jest-dom/vitest';
import { cleanup, fireEvent, render, screen, waitFor } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { MemoryRouter } from 'react-router-dom';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';

import { HardwareOverview } from '@/components/dashboard/hardware/hardware-overview';
import { TooltipProvider } from '@/components/ui/tooltip';
import { JointHomingState } from '@/gen/marengo/v1/marengo_pb';
import {
  activateUrdf,
  fetchCompleteness,
  uploadUrdf,
} from '@/lib/hardware-api';
import { useRobotStore } from '@/state/robotStore';

function wireJoint(
  name: string,
  overrides: {
    fault?: number;
    outOfLimits?: boolean;
    driveActive?: boolean;
    homingState?: JointHomingState;
    position?: number;
  } = {},
) {
  return {
    $typeName: 'marengo.v1.JointState' as const,
    name,
    position: overrides.position ?? 0,
    velocity: 0,
    effort: 0,
    temperatureC: 30,
    fault: overrides.fault ?? 0,
    homingState: overrides.homingState ?? JointHomingState.VERIFIED,
    driveActive: overrides.driveActive ?? false,
    outOfLimits: overrides.outOfLimits ?? false,
  };
}

vi.mock('@/lib/hardware-api', () => ({
  fetchCompleteness: vi.fn(async () => ({
    warnings: [
      {
        code: 'missing_mass',
        severity: 'warn',
        joint: 'right_elbow_pitch',
        message: 'gap for test',
      },
    ],
  })),
  uploadUrdf: vi.fn(),
  resolveUrdfPreview: vi.fn(),
  activateUrdf: vi.fn(),
  fetchUrdfArchiveList: vi.fn(async () => ({ entries: [] })),
  restoreUrdfArchive: vi.fn(),
  fetchLiveUrdf: vi.fn(),
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

vi.mock('@/lib/persist-joint-limits', () => ({
  persistJointLimits: vi.fn(),
}));

vi.mock('@/lib/gateway-api', () => ({
  postSetZeroCommand: vi.fn(),
  postEnableCommand: vi.fn(async () => undefined),
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

function renderHardware() {
  const client = new QueryClient({
    defaultOptions: { queries: { retry: false } },
  });
  return render(
    <QueryClientProvider client={client}>
      <MemoryRouter>
        <TooltipProvider>
          <HardwareOverview />
        </TooltipProvider>
      </MemoryRouter>
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
  vi.clearAllMocks();
});

beforeEach(() => {
  class ResizeObserverMock {
    observe() {}
    unobserve() {}
    disconnect() {}
  }
  vi.stubGlobal('ResizeObserver', ResizeObserverMock);
});

describe('HardwareOverview', () => {
  it('opts into pointer events under Strategy C main shell', () => {
    renderHardware();
    // dashboardMainPointerClassName is pointer-events-none; panels must re-enable.
    expect(screen.getByTestId('hardware-overview').className).toContain(
      'pointer-events-auto',
    );
  });

  it('renders table-first hardware workspace with warn badges', async () => {
    renderHardware();
    expect(screen.getByTestId('hardware-overview')).toBeTruthy();
    await waitFor(() => {
      expect(screen.getByTestId('hardware-row-right_shoulder_roll')).toBeTruthy();
    });
    expect(screen.getByTitle(/1 completeness gaps/i)).toBeTruthy();
    expect(screen.getByText(/gaps/)).toBeTruthy();
  });

  it('shows zero gaps only after completeness succeeds with no warnings', async () => {
    vi.mocked(fetchCompleteness).mockResolvedValueOnce({ warnings: [] });

    renderHardware();

    expect(await screen.findByText('0 gaps')).toBeTruthy();
    expect(screen.getByTitle('No completeness warnings')).toBeTruthy();
  });

  it('shows completeness unavailable when the fetch fails', async () => {
    vi.mocked(fetchCompleteness).mockRejectedValueOnce(
      new Error('hardware completeness failed: HTTP 503'),
    );

    renderHardware();

    expect(await screen.findByTitle('Completeness unavailable')).toBeTruthy();
    expect(screen.queryByText('0 gaps')).toBeNull();
    expect(screen.queryByTitle('No completeness warnings')).toBeNull();
  });

  it('does not disable Import when warnings are present', async () => {
    renderHardware();
    await waitFor(() => {
      expect(screen.getByTestId('hardware-import-btn')).toBeTruthy();
    });
    const importBtn = screen.getByTestId('hardware-import-btn') as HTMLButtonElement;
    expect(importBtn.disabled).toBe(false);
  });

  it('blocks Set Limits in sheet when motors ACTIVE', async () => {
    useRobotStore.setState({ connected: true, operationalMode: 'ACTIVE' });
    renderHardware();
    await waitFor(() => {
      expect(screen.getByTestId('hardware-row-right_shoulder_roll')).toBeTruthy();
    });
    fireEvent.click(screen.getByTestId('hardware-row-right_shoulder_roll'));
    await waitFor(() => {
      expect(screen.getByRole('dialog')).toBeTruthy();
    });
    const setLimits = screen.getByRole('button', { name: 'Set Limits' }) as HTMLButtonElement;
    expect(setLimits.disabled).toBe(true);
    expect(screen.getAllByText(/Disable motors first/i).length).toBeGreaterThan(0);
  });

  it('refuses URDF activation while motors are ACTIVE', async () => {
    useRobotStore.setState({ connected: true, operationalMode: 'ACTIVE' });
    vi.mocked(uploadUrdf).mockResolvedValueOnce({
      ok: true,
      upload_id: 'upload-active-gate',
      preview: {
        overlapping_joints: [],
        new_joints: [],
        field_diffs: [],
      },
    });

    renderHardware();
    fireEvent.click(screen.getByTestId('hardware-import-btn'));

    const file = new File(['<robot name="test" />'], 'test.urdf', {
      type: 'application/xml',
    });
    Object.defineProperty(file, 'text', {
      value: vi.fn(async () => '<robot name="test" />'),
    });
    fireEvent.change(await screen.findByTestId('import-file-input'), {
      target: { files: [file] },
    });

    const accept = await screen.findByTestId('import-accept') as HTMLButtonElement;
    expect(accept.disabled).toBe(true);
    expect(screen.getByText(/URDF activation refused while ACTIVE/i)).toBeTruthy();
    fireEvent.click(accept);
    expect(activateUrdf).not.toHaveBeenCalled();
  });

  it('reports when activation requires a marengo-pi restart', async () => {
    useRobotStore.setState({ connected: true, operationalMode: 'DISABLED' });
    vi.mocked(uploadUrdf).mockResolvedValueOnce({
      ok: true,
      upload_id: 'upload-restart',
      preview: {
        overlapping_joints: [],
        new_joints: [],
        field_diffs: [],
      },
    });
    vi.mocked(activateUrdf).mockResolvedValueOnce({
      ok: true,
      message: 'URDF activated.',
      checksum_sha256: 'abc123',
      completeness: { warnings: [] },
      restart_required: true,
    });

    renderHardware();
    fireEvent.click(screen.getByTestId('hardware-import-btn'));

    const file = new File(['<robot name="test" />'], 'test.urdf', {
      type: 'application/xml',
    });
    Object.defineProperty(file, 'text', {
      value: vi.fn(async () => '<robot name="test" />'),
    });
    fireEvent.change(await screen.findByTestId('import-file-input'), {
      target: { files: [file] },
    });

    const accept = await screen.findByTestId('import-accept');
    fireEvent.click(accept);

    expect(
      await screen.findByText(/marengo-pi must be restarted before the new URDF is enforced/i),
    ).toBeTruthy();
  });

  it('renders facet badges from wire with Fault priority over Active', async () => {
    useRobotStore.setState({
      connected: true,
      robotState: {
        $typeName: 'marengo.v1.RobotState',
        timestamp: { $typeName: 'google.protobuf.Timestamp', seconds: 0n, nanos: 0 },
        joints: [
          wireJoint('right_shoulder_roll', {
            driveActive: true,
            fault: 0x10,
            homingState: JointHomingState.VERIFIED,
          }),
          wireJoint('right_shoulder_pitch', {
            homingState: JointHomingState.VERIFIED,
          }),
        ],
      } as never,
    });

    renderHardware();
    const roll = await screen.findByTestId('hardware-row-right_shoulder_roll');
    expect(roll.querySelector('[data-testid="commissioning-badge"]')?.textContent).toMatch(
      /Fault/i,
    );
    const pitch = screen.getByTestId('hardware-row-right_shoulder_pitch');
    expect(pitch.querySelector('[data-testid="commissioning-badge"]')?.textContent).toMatch(
      /Ready/i,
    );
  });

  it('shows Ready from wire Verified — not browser zero marks', async () => {
    useRobotStore.setState({
      connected: true,
      robotState: {
        $typeName: 'marengo.v1.RobotState',
        timestamp: { $typeName: 'google.protobuf.Timestamp', seconds: 0n, nanos: 0 },
        joints: [
          wireJoint('right_shoulder_roll', {
            homingState: JointHomingState.UNHOMED,
          }),
          wireJoint('right_shoulder_pitch', {
            homingState: JointHomingState.VERIFIED,
          }),
        ],
      } as never,
    });

    renderHardware();
    const roll = await screen.findByTestId('hardware-row-right_shoulder_roll');
    expect(roll.querySelector('[data-testid="commissioning-badge"]')?.textContent).toMatch(
      /Online/i,
    );
    expect(roll.querySelector('[data-testid="commissioning-badge"]')?.textContent).not.toMatch(
      /Ready/i,
    );
    const pitch = screen.getByTestId('hardware-row-right_shoulder_pitch');
    expect(pitch.querySelector('[data-testid="commissioning-badge"]')?.textContent).toMatch(
      /Ready/i,
    );
  });

  it('renders limb and robot aggregation summary', async () => {
    useRobotStore.setState({
      connected: true,
      robotState: {
        $typeName: 'marengo.v1.RobotState',
        timestamp: { $typeName: 'google.protobuf.Timestamp', seconds: 0n, nanos: 0 },
        joints: [
          wireJoint('right_shoulder_roll'),
          wireJoint('right_shoulder_pitch'),
        ],
      } as never,
    });

    renderHardware();
    expect(await screen.findByTestId('commissioning-aggregation')).toBeTruthy();
    expect(screen.getByTestId('robot-ready-badge')).toBeTruthy();
    expect(screen.getByTestId('limb-ready-right_arm')).toBeTruthy();
  });

  it('keeps Enable disabled without live wire facets; Set Limits still opens', async () => {
    useRobotStore.setState({
      connected: true,
      operationalMode: 'DISABLED',
      robotState: null,
    });
    renderHardware();
    await waitFor(() => {
      expect(screen.getByTestId('hardware-row-right_shoulder_roll')).toBeTruthy();
    });

    const enable = screen.getByTestId('hardware-enable-ready-in-scope') as HTMLButtonElement;
    expect(enable.disabled).toBe(true);
    expect(enable.textContent).toMatch(/Enable/i);

    fireEvent.click(screen.getByTestId('hardware-row-right_shoulder_roll'));
    await waitFor(() => {
      expect(screen.getByRole('dialog')).toBeTruthy();
    });
    expect(screen.getByRole('button', { name: 'Set Limits' })).toBeTruthy();
    expect(screen.getByRole('button', { name: 'Set Zero' })).toBeTruthy();
    const setLimits = screen.getByRole('button', { name: 'Set Limits' }) as HTMLButtonElement;
    expect(setLimits.disabled).toBe(false);
  });

  it('posts Enable when wire facets are live', async () => {
    const { postEnableCommand } = await import('@/lib/gateway-api');
    useRobotStore.setState({
      connected: true,
      operationalMode: 'DISABLED',
      robotState: {
        $typeName: 'marengo.v1.RobotState',
        timestamp: { $typeName: 'google.protobuf.Timestamp', seconds: 0n, nanos: 0 },
        joints: [
          wireJoint('right_shoulder_roll'),
          wireJoint('right_shoulder_pitch'),
        ],
      } as never,
    });

    renderHardware();
    const enable = (await screen.findByTestId(
      'hardware-enable-ready-in-scope',
    )) as HTMLButtonElement;
    expect(enable.disabled).toBe(false);
    fireEvent.click(enable);
    await waitFor(() => {
      expect(postEnableCommand).toHaveBeenCalledWith(true);
    });
  });
});
