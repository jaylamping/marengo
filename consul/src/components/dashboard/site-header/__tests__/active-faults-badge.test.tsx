// @vitest-environment jsdom
import '@testing-library/jest-dom/vitest';
import { create } from '@bufbuild/protobuf';
import { cleanup, render, screen } from '@testing-library/react';
import { MemoryRouter } from 'react-router-dom';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { afterEach, describe, expect, it } from 'vitest';

import { SiteHeaderStatusBadges } from '@/components/dashboard/site-header/site-header-status-badges';
import { TooltipProvider } from '@/components/ui/tooltip';
import {
  FaultSchema,
  FaultSeverity,
  OperationalMode,
  SafetyStateSchema,
} from '@/gen/marengo/v1/marengo_pb';
import { useRobotStore } from '@/state/robotStore';

function renderBadges() {
  return render(
    <QueryClientProvider client={new QueryClient()}>
      <MemoryRouter>
        <TooltipProvider>
          <SiteHeaderStatusBadges />
        </TooltipProvider>
      </MemoryRouter>
    </QueryClientProvider>,
  );
}

afterEach(() => {
  cleanup();
  useRobotStore.setState({ safetyState: null });
});

describe('header active faults (UI-07)', () => {
  it('lists each active fault with joint, code and message', () => {
    useRobotStore.setState({
      safetyState: create(SafetyStateSchema, {
        mode: OperationalMode.DISABLED,
        activeFaults: [
          create(FaultSchema, {
            code: 'over_temp',
            message: 'casing 82 C',
            severity: FaultSeverity.ESTOP,
            joint: 'right_elbow_pitch',
          }),
        ],
      }),
    });
    renderBadges();
    const badge = screen.getByTestId('active-faults');
    expect(badge).toHaveTextContent('1 FAULT');
    expect(badge).toHaveAccessibleName(
      'Active faults: E-STOP right_elbow_pitch: [over_temp] casing 82 C',
    );
  });

  it('shows nothing when there are no active faults', () => {
    useRobotStore.setState({
      safetyState: create(SafetyStateSchema, { mode: OperationalMode.DISABLED }),
    });
    renderBadges();
    expect(screen.queryByTestId('active-faults')).toBeNull();
  });
});
