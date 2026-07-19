// @vitest-environment jsdom
import { describe, it, expect, vi, afterEach } from 'vitest';
import { cleanup, render, screen } from '@testing-library/react';

import { SectionCards } from '@/components/dashboard/section-cards';
import { sectionCardsGridClassName } from '@/components/dashboard/layout/constants';

vi.mock('@/components/dashboard/cards/pi-host-card', () => ({
  PiHostCard: () => <div data-testid="pi-host-card" />,
}));

vi.mock('@/components/dashboard/cards/jetson-host-card', () => ({
  JetsonHostCard: () => <div data-testid="jetson-host-card" />,
}));

vi.mock('@/components/dashboard/cards/power-system-card', () => ({
  PowerSystemCard: () => <div data-testid="power-system-card" />,
}));

afterEach(() => {
  cleanup();
});

describe('SectionCards (overview panel grid)', () => {
  it('uses the shared section grid class without legacy gradient selectors', () => {
    expect(sectionCardsGridClassName).toContain('grid');
    expect(sectionCardsGridClassName).toContain('grid-cols-3');
    expect(sectionCardsGridClassName).not.toContain('bg-linear-to-t');
    expect(sectionCardsGridClassName).not.toContain('from-primary/5');
  });

  it('renders the three live overview host cards', () => {
    render(<SectionCards />);

    const grid = screen.getByTestId('section-cards-grid');
    expect(grid.className).toBe(sectionCardsGridClassName);
    expect(screen.getByTestId('pi-host-card')).toBeTruthy();
    expect(screen.getByTestId('jetson-host-card')).toBeTruthy();
    expect(screen.getByTestId('power-system-card')).toBeTruthy();
    expect(screen.queryByTestId('placeholder-card')).toBeNull();
  });
});
