// @vitest-environment jsdom
import { afterEach, describe, expect, it, vi } from 'vitest';
import { cleanup, render, screen } from '@testing-library/react';

import { OverviewPosturePanel } from '@/components/dashboard/overview/overview-posture-panel';
import { dashboardOverviewHeroClassName } from '@/components/dashboard/layout/constants';

vi.mock('@react-three/fiber', () => ({
  Canvas: ({ children }: { children: React.ReactNode }) => (
    <div data-testid="r3f-canvas">{children}</div>
  ),
}));

vi.mock('@react-three/drei', () => ({
  OrbitControls: () => null,
}));

vi.mock('@/lib/chappe-config', () => ({
  isChappeLive: () => false,
}));

vi.mock('@/state/robotStore', () => ({
  useRobotStore: (sel: (s: { connected: boolean }) => unknown) =>
    sel({ connected: false }),
}));

afterEach(() => {
  cleanup();
});

describe('OverviewPosturePanel', () => {
  it('labels wireframe feed as demo pose', () => {
    render(<OverviewPosturePanel />);
    expect(screen.getByTestId('overview-posture-panel')).toBeTruthy();
    expect(screen.getByText(/Demo pose/i)).toBeTruthy();
  });
});

describe('overview hero layout constants', () => {
  it('uses a two-column hero above the KPI strip', () => {
    expect(dashboardOverviewHeroClassName).toContain('grid');
    expect(dashboardOverviewHeroClassName).toContain('@3xl/main:grid-cols-2');
  });
});
