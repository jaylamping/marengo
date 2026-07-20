// @vitest-environment jsdom
import type { ReactElement } from 'react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { cleanup, fireEvent, render, screen, within } from '@testing-library/react';

import { SetLimitsPanel } from '@/components/dashboard/inventory/set-limits-panel';
import { TooltipProvider } from '@/components/ui/tooltip';
import { useLimitListenStore } from '@/state/limitListenStore';
import { useRobotStore } from '@/state/robotStore';

function renderPanel(ui: ReactElement) {
  return render(<TooltipProvider>{ui}</TooltipProvider>);
}

vi.mock('@/lib/gateway-api', () => ({
  postSetZeroCommand: vi.fn(async () => undefined),
}));

import { postSetZeroCommand } from '@/lib/gateway-api';

afterEach(() => {
  cleanup();
  useLimitListenStore.getState().reset();
  useRobotStore.setState({ connected: false, operationalMode: null });
  vi.mocked(postSetZeroCommand).mockClear();
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
    expect(screen.getByText(/Disable motors first/i)).toBeTruthy();
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
    await vi.waitFor(() => {
      expect(postSetZeroCommand).toHaveBeenCalledWith('right_shoulder_pitch');
    });
  });

  it('runs listen → review → apply with seeded samples', () => {
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

    useLimitListenStore.getState().ingestPosition('right_shoulder_pitch', -0.5);
    useLimitListenStore.getState().ingestPosition('right_shoulder_pitch', 1.2);

    fireEvent.click(screen.getByRole('button', { name: 'Stop' }));
    expect(screen.getByText('Review')).toBeTruthy();
    expect(screen.getByText(/Proposed range/i)).toBeTruthy();

    fireEvent.click(screen.getByRole('button', { name: 'Apply Limits' }));
    expect(onApplyRange).toHaveBeenCalledWith('−0.50–1.20');
  });
});
