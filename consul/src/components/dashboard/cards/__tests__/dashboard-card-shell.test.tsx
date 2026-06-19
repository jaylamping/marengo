// @vitest-environment jsdom
import { describe, it, expect, afterEach } from 'vitest';
import { cleanup, render, screen } from '@testing-library/react';

import { DashboardCardShell } from '@/components/dashboard/cards/dashboard-card-shell';
import { dashboardGlassCardClassName } from '@/components/dashboard/layout/constants';
import { cardVariants } from '@/components/ui/card';

afterEach(() => {
  cleanup();
});

describe('DashboardCardShell (overview glass)', () => {
  it('uses cardVariants glass styling tokens', () => {
    const classes = cardVariants({ variant: 'glass' });
    expect(classes).toContain('backdrop-blur-xl');
    expect(classes).toContain('[border-top-color:var(--glass-refraction-top)]');
  });

  it('renders a glass card shell with panel pointer events', () => {
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
    expect(card?.className).toMatch(/backdrop-blur-xl/);
    expect(card?.className).toContain(dashboardGlassCardClassName);
  });

  it('does not apply glass styling when variant is default', () => {
    const defaultClasses = cardVariants({ variant: 'default' });
    const glassClasses = cardVariants({ variant: 'glass' });
    expect(defaultClasses).not.toContain('backdrop-blur-xl');
    expect(glassClasses).not.toEqual(defaultClasses);
  });
});
