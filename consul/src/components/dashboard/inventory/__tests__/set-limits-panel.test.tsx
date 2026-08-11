// @vitest-environment jsdom
import type { ReactElement } from 'react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { cleanup, fireEvent, render, screen, within } from '@testing-library/react';

import { SetLimitsPanel } from '@/components/dashboard/inventory/set-limits-panel';
import { TooltipProvider } from '@/components/ui/tooltip';
import { useLimitListenStore } from '@/state/limitListenStore';
import { useNeedsRestartStore } from '@/state/needsRestartStore';
import { useRobotStore } from '@/state/robotStore';

function renderPanel(ui: ReactElement) {
  return render(<TooltipProvider>{ui}</TooltipProvider>);
}

vi.mock('@/lib/gateway-api', () => ({
  postSetZeroCommand: vi.fn(async () => undefined),
  fetchActuatorLimits: vi.fn(async () => null),
}));

vi.mock('@/lib/persist-joint-limits', () => ({
  persistJointLimits: vi.fn(async () => ({
    ok: true,
    lower: -0.53,
    upper: 1.23,
    softLower: -0.503,
    softUpper: 1.203,
    restartRequired: false,
    persistStatus: 'durable',
    localSync: 'skipped',
    message: 'Updated right_shoulder_pitch',
  })),
}));

vi.mock('@/lib/query-client', () => ({
  queryClient: {
    setQueryData: vi.fn(),
    invalidateQueries: vi.fn(async () => undefined),
  },
}));

import { postSetZeroCommand } from '@/lib/gateway-api';
import { persistJointLimits } from '@/lib/persist-joint-limits';
import { queryClient } from '@/lib/query-client';

afterEach(() => {
  cleanup();
  useLimitListenStore.getState().reset();
  useNeedsRestartStore.setState({
    pending: [],
    restartDialogOpen: false,
    dialogReason: null,
  });
  useRobotStore.setState({ connected: false, operationalMode: null });
  vi.mocked(postSetZeroCommand).mockClear();
  vi.mocked(persistJointLimits).mockClear();
  vi.mocked(queryClient.setQueryData).mockClear();
  vi.mocked(queryClient.invalidateQueries).mockClear();
});

describe('SetLimitsPanel', () => {
  it('shows Set Limits for an actuator and blocks start without Chappe', () => {
    renderPanel(
      <SetLimitsPanel
        jointName="right_shoulder_pitch"
        currentLimit="−0.90–3.17"
        onApplyRange={vi.fn()}
      />,
    );
    expect(screen.getByTestId('set-limits-panel')).toBeTruthy();
    expect(
      (screen.getByRole('button', { name: 'Set Limits' }) as HTMLButtonElement)
        .disabled,
    ).toBe(true);
    expect(screen.getByText(/Connect Chappe/i)).toBeTruthy();
    expect(screen.queryByText(/Demo/i)).toBeNull();
  });

  it('enables Set Limits when live and not ACTIVE', () => {
    useRobotStore.setState({ connected: true, operationalMode: 'DISABLED' });
    renderPanel(
      <SetLimitsPanel
        jointName="right_shoulder_pitch"
        currentLimit="±1.57"
        onApplyRange={vi.fn()}
      />,
    );
    expect(
      (screen.getByRole('button', { name: 'Set Limits' }) as HTMLButtonElement)
        .disabled,
    ).toBe(false);
    expect(
      screen.getByRole('button', { name: 'Set Limits help' }),
    ).toBeTruthy();
    expect(screen.queryByText(/Support the assembly/i)).toBeNull();
  });

  it('blocks Set Limits when ACTIVE', () => {
    useRobotStore.setState({ connected: true, operationalMode: 'ACTIVE' });
    renderPanel(
      <SetLimitsPanel
        jointName="right_shoulder_pitch"
        currentLimit="±1.57"
        onApplyRange={vi.fn()}
      />,
    );
    expect(
      (screen.getByRole('button', { name: 'Set Limits' }) as HTMLButtonElement)
        .disabled,
    ).toBe(true);
    expect(
      screen.getAllByText(/Disable motors first/i).length,
    ).toBeGreaterThanOrEqual(1);
  });

  it('blocks Set Zero when ACTIVE', () => {
    useRobotStore.setState({ connected: true, operationalMode: 'ACTIVE' });
    renderPanel(
      <SetLimitsPanel
        jointName="right_shoulder_pitch"
        currentLimit="±1.57"
        onApplyRange={vi.fn()}
      />,
    );
    expect(
      (screen.getByRole('button', { name: 'Set Zero' }) as HTMLButtonElement)
        .disabled,
    ).toBe(true);
    expect(screen.getByText(/Set Zero refused while ACTIVE/i)).toBeTruthy();
  });

  it('shows Set Zero as destructive and requires dialog confirm before posting', async () => {
    useRobotStore.setState({ connected: true, operationalMode: 'DISABLED' });

    renderPanel(
      <SetLimitsPanel
        jointName="right_shoulder_pitch"
        currentLimit="±1.57"
        onApplyRange={vi.fn()}
      />,
    );
    const zero = screen.getByRole('button', { name: 'Set Zero' });
    expect(zero.className).toContain('bg-destructive');

    fireEvent.click(zero);
    const dialog = await screen.findByRole('alertdialog');
    expect(dialog).toBeTruthy();
    fireEvent.click(screen.getByRole('button', { name: 'Cancel' }));
    expect(postSetZeroCommand).not.toHaveBeenCalled();
    expect(screen.queryByRole('alertdialog')).toBeNull();

    fireEvent.click(screen.getByRole('button', { name: 'Set Zero' }));
    const confirmDialog = await screen.findByRole('alertdialog');
    fireEvent.click(
      within(confirmDialog).getByRole('button', { name: 'Confirm Set Zero' }),
    );
    expect(postSetZeroCommand).not.toHaveBeenCalled();
    fireEvent.click(
      within(confirmDialog).getByRole('checkbox', {
        name: /Sign\/direction checked/i,
      }),
    );
    fireEvent.click(
      within(confirmDialog).getByRole('button', { name: 'Confirm Set Zero' }),
    );
    await vi.waitFor(() => {
      expect(postSetZeroCommand).toHaveBeenCalledWith('right_shoulder_pitch', {
        signTestPassed: true,
      });
    });
    await vi.waitFor(() => {
      expect(screen.getByText(/Set Zero queued/i)).toBeTruthy();
    });
  });

  it('runs listen → review → apply with exact measured bounds', async () => {
    useRobotStore.setState({ connected: true, operationalMode: 'DISABLED' });
    const onApplyRange = vi.fn();
    renderPanel(
      <SetLimitsPanel
        jointName="right_shoulder_pitch"
        currentLimit="±1.57"
        onApplyRange={onApplyRange}
      />,
    );

    fireEvent.click(screen.getByRole('button', { name: 'Set Limits' }));
    expect(screen.getByText('Listening')).toBeTruthy();

    const store = useLimitListenStore.getState();
    for (const pos of [-0.5, -0.4, 0.0, 0.8, 1.2]) {
      store.ingestPosition('right_shoulder_pitch', pos);
    }

    fireEvent.click(screen.getByRole('button', { name: 'Stop' }));
    expect(screen.getByText('Review')).toBeTruthy();
    expect(screen.getByText(/Proposed range/i)).toBeTruthy();

    fireEvent.click(screen.getByRole('button', { name: 'Apply Limits' }));
    await vi.waitFor(() => {
      expect(persistJointLimits).toHaveBeenCalledWith('right_shoulder_pitch', {
        lower: -0.5,
        upper: 1.2,
      });
    });
    await vi.waitFor(() => {
      expect(queryClient.setQueryData).toHaveBeenCalled();
      expect(queryClient.invalidateQueries).toHaveBeenCalled();
      expect(onApplyRange).toHaveBeenCalledWith('−0.50–1.20');
      expect(useNeedsRestartStore.getState().pending).toEqual([]);
      expect(useNeedsRestartStore.getState().restartDialogOpen).toBe(false);
    });
  });

  it('keeps review and skips onApplyRange when gateway persist fails', async () => {
    vi.mocked(persistJointLimits).mockResolvedValueOnce({
      ok: false,
      message: 'Gateway rejected the limits patch',
    });
    useRobotStore.setState({ connected: true, operationalMode: 'DISABLED' });
    const onApplyRange = vi.fn();
    renderPanel(
      <SetLimitsPanel
        jointName="right_shoulder_pitch"
        currentLimit="±1.57"
        onApplyRange={onApplyRange}
      />,
    );

    fireEvent.click(screen.getByRole('button', { name: 'Set Limits' }));
    const store = useLimitListenStore.getState();
    for (const pos of [-0.5, -0.4, 0.0, 0.8, 1.2]) {
      store.ingestPosition('right_shoulder_pitch', pos);
    }
    fireEvent.click(screen.getByRole('button', { name: 'Stop' }));
    fireEvent.click(screen.getByRole('button', { name: 'Apply Limits' }));

    await vi.waitFor(() => {
      expect(screen.getByText(/Gateway rejected the limits patch/i)).toBeTruthy();
    });
    expect(onApplyRange).not.toHaveBeenCalled();
    expect(screen.getByText('Review')).toBeTruthy();
  });

  it('keeps review when persistJointLimits rejects', async () => {
    vi.mocked(persistJointLimits).mockRejectedValueOnce(new Error('network down'));
    useRobotStore.setState({ connected: true, operationalMode: 'DISABLED' });
    const onApplyRange = vi.fn();
    renderPanel(
      <SetLimitsPanel
        jointName="right_shoulder_pitch"
        currentLimit="±1.57"
        onApplyRange={onApplyRange}
      />,
    );

    fireEvent.click(screen.getByRole('button', { name: 'Set Limits' }));
    const store = useLimitListenStore.getState();
    for (const pos of [-0.5, -0.4, 0.0, 0.8, 1.2]) {
      store.ingestPosition('right_shoulder_pitch', pos);
    }
    fireEvent.click(screen.getByRole('button', { name: 'Stop' }));
    fireEvent.click(screen.getByRole('button', { name: 'Apply Limits' }));

    await vi.waitFor(() => {
      expect(screen.getByText(/network down/i)).toBeTruthy();
    });
    expect(onApplyRange).not.toHaveBeenCalled();
    expect(screen.getByText('Review')).toBeTruthy();
    expect(
      (screen.getByRole('button', { name: 'Apply Limits' }) as HTMLButtonElement)
        .disabled,
    ).toBe(false);
  });
});
