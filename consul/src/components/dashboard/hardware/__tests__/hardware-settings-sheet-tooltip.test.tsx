// @vitest-environment jsdom
import '@testing-library/jest-dom/vitest';
import { cleanup, fireEvent, render, screen } from '@testing-library/react';
import { afterEach, beforeAll, describe, expect, it, vi } from 'vitest';
import { MemoryRouter } from 'react-router-dom';

import { HardwareSettingsSheet } from '@/components/dashboard/hardware/hardware-settings-sheet';
import type { HardwareJointRow } from '@/components/dashboard/hardware/build-hardware-rows';
import { TooltipProvider } from '@/components/ui/tooltip';
import { useLimitListenStore } from '@/state/limitListenStore';
import { useRobotStore } from '@/state/robotStore';

vi.mock('@/hooks/use-active-reporting-lease', () => ({
  useActiveReportingLease: () => 'idle',
}));

vi.mock('@/lib/gateway-api', () => ({
  postSetZeroCommand: vi.fn(async () => undefined),
  fetchActuatorLimits: vi.fn(async () => null),
}));

vi.mock('@/lib/persist-joint-limits', () => ({
  persistJointLimits: vi.fn(async () => ({
    ok: true,
    lower: -0.5,
    upper: 1.2,
    restartRequired: false,
    message: 'ok',
  })),
}));

vi.mock('@/lib/query-client', () => ({
  queryClient: {
    invalidateQueries: vi.fn(async () => undefined),
  },
}));

const row: HardwareJointRow = {
  joint: 'right_shoulder_pitch',
  onCan: true,
  canId: 1,
  canInterface: 'can0',
  motorType: 'rs03',
  warningCount: 0,
  warnings: [],
  liveRange: '−0.90–3.17',
  diskHardLower: -0.9,
  diskHardUpper: 3.17,
  diskSoftLower: -0.85,
  diskSoftUpper: 3.12,
  direction: 1,
  badge: 'Ready',
  facet: {
    name: 'right_shoulder_pitch',
    online: true,
    motorMapped: true,
    fault: 0,
    outOfLimits: false,
    driveActive: false,
    homingState: undefined,
  },
};

function renderOpenSheet() {
  return render(
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
}

beforeAll(() => {
  vi.stubGlobal(
    'ResizeObserver',
    class {
      observe() {}
      unobserve() {}
      disconnect() {}
    },
  );
});

afterEach(() => {
  cleanup();
  useLimitListenStore.getState().reset();
  useRobotStore.setState({ connected: true, operationalMode: 'DISABLED' });
});

describe('HardwareSettingsSheet Set Limits help tooltip', () => {
  it('does not open on sheet auto-focus; opens when the help control is focused', async () => {
    renderOpenSheet();

    const help = await screen.findByRole('button', { name: 'Set Limits help' });
    expect(help).not.toHaveFocus();
    expect(screen.queryByRole('tooltip')).toBeNull();

    fireEvent.focus(help);
    expect(await screen.findByRole('tooltip')).toHaveTextContent(
      /Hardware holds an Active Reporting lease/i,
    );
  });
});
