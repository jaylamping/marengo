// @vitest-environment jsdom
import { describe, it, expect, afterEach } from 'vitest';
import { cleanup, render, screen } from '@testing-library/react';

import { DashboardCardShell } from '@/components/dashboard/cards/dashboard-card-shell';
import { dashboardPanelCardClassName } from '@/components/dashboard/layout/constants';
import { cardVariants } from '@/components/ui/card';

afterEach(() => {
  cleanup();
});

describe('DashboardCardShell (overview panel)', () => {
  it('uses cardVariants panel styling tokens', () => {
    const classes = cardVariants({ variant: 'panel' });
    expect(classes).toContain('border-line');
    expect(classes).toContain('bg-surface-1');
    expect(classes).not.toContain('backdrop-blur');
  });

  it('renders a panel card shell with panel pointer events', () => {
    render(
      <DashboardCardShell
        title="PI 5"
        description="Bench host"
        footerPrimary="Uptime 2h 15m"
        footerSecondary="Host metrics live"
      />,
    );

    const card = screen.getByText('PI 5').closest('[data-slot="card"]');
    expect(card).toBeTruthy();
    expect(card?.className).toContain('bg-surface-1');
    expect(card?.className).toContain(dashboardPanelCardClassName);
  });

  it('does not apply panel styling when variant is default', () => {
    const defaultClasses = cardVariants({ variant: 'default' });
    const panelClasses = cardVariants({ variant: 'panel' });
    expect(defaultClasses).not.toContain('border-line');
    expect(panelClasses).not.toEqual(defaultClasses);
  });
});
