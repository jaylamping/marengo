// @vitest-environment jsdom
import { describe, it, expect, afterEach } from 'vitest';
import { cleanup, render } from '@testing-library/react';

import { ChartSectionSkeleton } from '@/components/dashboard/overview/chart-section-skeleton';
import { dashboardGlassCardClassName } from '@/components/dashboard/layout/constants';

afterEach(() => {
  cleanup();
});

describe('ChartSectionSkeleton (glass shell)', () => {
  it('renders a glass variant loading card with panel pointer events', () => {
    const { container } = render(<ChartSectionSkeleton />);

    const card = container.querySelector('[data-slot="card"]');
    expect(card).toBeTruthy();
    expect(card?.className).toMatch(/backdrop-blur-xl/);
    expect(card?.className).toContain(dashboardGlassCardClassName);
  });
});
