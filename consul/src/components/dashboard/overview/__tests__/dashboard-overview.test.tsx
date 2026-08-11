// @vitest-environment jsdom
import { afterEach, describe, expect, it, vi } from 'vitest';
import { cleanup, render, screen } from '@testing-library/react';

import { DashboardOverview } from '@/components/dashboard/overview/dashboard-overview';
import { dashboardOverviewCanClassName } from '@/components/dashboard/layout/constants';

vi.mock('@/components/dashboard/section-cards', () => ({
  SectionCards: () => <div data-testid="section-cards-grid" />,
}));

vi.mock('@/components/dashboard/overview/can-bus-spectrum-panel', () => ({
  CanBusSpectrumPanel: () => <div data-testid="overview-can-bus-panel" />,
}));

vi.mock('@/components/dashboard/layout/deferred-mount', () => ({
  DeferredMount: ({ children }: { children: React.ReactNode }) => <>{children}</>,
}));

afterEach(() => {
  cleanup();
});

describe('DashboardOverview layout', () => {
  it('puts host cards above a full-width CAN section and omits posture', async () => {
    render(<DashboardOverview active />);
    expect(screen.getByTestId('dashboard-overview')).toBeTruthy();
    expect(screen.getByTestId('section-cards-grid')).toBeTruthy();
    expect(screen.getByTestId('overview-can-section')).toBeTruthy();
    expect(await screen.findByTestId('overview-can-bus-panel')).toBeTruthy();
    expect(screen.queryByTestId('overview-posture-panel')).toBeNull();
    expect(screen.queryByTestId('overview-hero')).toBeNull();
    expect(dashboardOverviewCanClassName).toContain('px-4');
    expect(dashboardOverviewCanClassName).not.toContain('grid-cols-2');
  });
});
