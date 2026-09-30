// @vitest-environment jsdom
import '@testing-library/jest-dom/vitest';
import { cleanup, render, screen } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { MemoryRouter } from 'react-router-dom';

import { HardwareSettingsSheet } from '@/components/dashboard/hardware/hardware-settings-sheet';
import type { HardwareJointRow } from '@/components/dashboard/hardware/build-hardware-rows';
import { TooltipProvider } from '@/components/ui/tooltip';
import { useRobotStore } from '@/state/robotStore';

const leaseMock = vi.fn();

vi.mock('@/hooks/use-active-reporting-lease', () => ({
  useActiveReportingLease: (opts: { joint: string | null; enabled: boolean }) =>
    leaseMock(opts),
}));

vi.mock('@/components/dashboard/inventory/set-limits-panel', () => ({
  SetLimitsPanel: ({ jointName }: { jointName: string }) => (
    <div data-testid="set-limits-panel">limits:{jointName}</div>
  ),
}));

const row: HardwareJointRow = {
  joint: 'right_shoulder_roll',
  onCan: true,
  canId: 1,
  canInterface: 'can0',
  motorType: 'rs03',
  warningCount: 0,
  warnings: [],
  liveRange: '−0.10–0.25',
  diskHardLower: -0.099,
  diskHardUpper: 0.248,
  diskSoftLower: -0.072,
  diskSoftUpper: 0.221,
  direction: 1,
  badge: 'Ready',
  facet: {
    name: 'right_shoulder_roll',
    online: true,
    motorMapped: true,
    fault: 0,
    outOfLimits: false,
    driveActive: false,
    homingState: undefined,
  },
};

afterEach(() => {
  cleanup();
  leaseMock.mockReset();
  useRobotStore.setState({ connected: true, operationalMode: 'DISABLED' });
});

describe('HardwareSettingsSheet', () => {
  it('holds an active-reporting lease while open for a CAN joint', () => {
    leaseMock.mockReturnValue('requested');
    render(
      <MemoryRouter>
        <TooltipProvider>
          <HardwareSettingsSheet
            row={row}
            open
            onOpenChange={() => undefined}
            onApplyRange={() => undefined}
          />
        </TooltipProvider>
      </MemoryRouter>,
    );
    expect(leaseMock).toHaveBeenCalledWith({
      joint: 'right_shoulder_roll',
      enabled: true,
    });
    expect(screen.getByTestId('hardware-enhanced-logging')).toBeTruthy();
    expect(screen.getByText(/replaces the durable hard\/soft SoT/i)).toBeTruthy();
    expect(screen.getByTestId('set-limits-panel')).toBeTruthy();
    const commissioning = screen.getByTestId('hardware-commissioning-commands');
    const limitsHeading = screen.getByRole('heading', {
      name: /Limits \(ADR 0012\)/i,
    });
    expect(
      commissioning.compareDocumentPosition(limitsHeading) &
        Node.DOCUMENT_POSITION_FOLLOWING,
    ).toBeTruthy();
  });

  it('does not request a lease when the sheet is closed', () => {
    leaseMock.mockReturnValue('idle');
    render(
      <MemoryRouter>
        <TooltipProvider>
          <HardwareSettingsSheet
            row={row}
            open={false}
            onOpenChange={() => undefined}
            onApplyRange={() => undefined}
          />
        </TooltipProvider>
      </MemoryRouter>,
    );
    expect(leaseMock).toHaveBeenCalledWith({
      joint: null,
      enabled: false,
    });
  });
});
