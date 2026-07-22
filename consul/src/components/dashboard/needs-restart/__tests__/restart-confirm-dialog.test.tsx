// @vitest-environment jsdom
import { cleanup, fireEvent, render, screen } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

import { RestartConfirmDialog } from '@/components/dashboard/needs-restart/restart-confirm-dialog';
import { NeedsRestartBadge } from '@/components/dashboard/needs-restart/needs-restart-badge';
import { useNeedsRestartStore } from '@/state/needsRestartStore';
import { useRobotStore } from '@/state/robotStore';

vi.mock('@/lib/config-api', () => ({
  restartMarengoPi: vi.fn(async () => ({ ok: true, message: 'restarted' })),
}));

import { restartMarengoPi } from '@/lib/config-api';

afterEach(() => {
  cleanup();
});

beforeEach(() => {
  useNeedsRestartStore.setState({
    pending: [
      {
        profile: 'arm_4dof_right',
        joint: 'right_elbow_pitch',
        reason: 'structural',
        expected_revision: 'r1',
      },
    ],
    restartDialogOpen: false,
    dialogReason: null,
  });
  useRobotStore.setState({ connected: true, operationalMode: 'DISABLED' });
  vi.mocked(restartMarengoPi).mockClear();
  vi.mocked(restartMarengoPi).mockResolvedValue({
    ok: true,
    message: 'restarted',
  });
});

describe('RestartConfirmDialog + NeedsRestartBadge', () => {
  it('badge opens dialog; Later keeps pending', async () => {
    render(
      <>
        <NeedsRestartBadge />
        <RestartConfirmDialog />
      </>,
    );
    fireEvent.click(screen.getByTestId('needs-restart-badge'));
    expect(screen.getByTestId('restart-confirm-dialog')).toBeTruthy();
    fireEvent.click(screen.getByRole('button', { name: 'Later' }));
    await vi.waitFor(() => {
      expect(useNeedsRestartStore.getState().restartDialogOpen).toBe(false);
    });
    expect(useNeedsRestartStore.getState().pending.map((p) => p.joint)).toEqual(
      ['right_elbow_pitch'],
    );
    expect(restartMarengoPi).not.toHaveBeenCalled();
  });

  it('Restart now clears pending on success', async () => {
    useNeedsRestartStore.getState().openRestartDialog({ reason: 'structural' });
    render(<RestartConfirmDialog />);
    fireEvent.click(screen.getByTestId('restart-now-button'));
    await vi.waitFor(() => {
      expect(restartMarengoPi).toHaveBeenCalled();
      expect(useNeedsRestartStore.getState().pending).toEqual([]);
    });
  });

  it('failed restart keeps pending', async () => {
    vi.mocked(restartMarengoPi).mockResolvedValueOnce({
      ok: false,
      message: 'boom',
    });
    useNeedsRestartStore.getState().openRestartDialog({ reason: 'structural' });
    render(<RestartConfirmDialog />);
    fireEvent.click(screen.getByTestId('restart-now-button'));
    await vi.waitFor(() => {
      expect(screen.getByText(/boom/i)).toBeTruthy();
    });
    expect(useNeedsRestartStore.getState().pending.map((p) => p.joint)).toEqual(
      ['right_elbow_pitch'],
    );
  });

  it('disables Restart now when ACTIVE', () => {
    useRobotStore.setState({ connected: true, operationalMode: 'ACTIVE' });
    useNeedsRestartStore.getState().openRestartDialog();
    render(<RestartConfirmDialog />);
    expect(
      (screen.getByTestId('restart-now-button') as HTMLButtonElement).disabled,
    ).toBe(true);
    expect(screen.getByTestId('restart-active-block')).toBeTruthy();
  });

  it('pending row badge only shows for that joint', () => {
    render(
      <>
        <NeedsRestartBadge variant="pending" jointName="right_elbow_pitch" />
        <NeedsRestartBadge variant="pending" jointName="right_shoulder_pitch" />
      </>,
    );
    expect(screen.getByTestId('pending-restart-badge')).toBeTruthy();
    expect(screen.getAllByTestId('pending-restart-badge')).toHaveLength(1);
  });
});
