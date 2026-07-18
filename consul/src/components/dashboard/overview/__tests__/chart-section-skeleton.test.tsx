// @vitest-environment jsdom
import { describe, it, expect, afterEach } from 'vitest';
import { cleanup, render } from '@testing-library/react';

import { ChartSectionSkeleton } from '@/components/dashboard/overview/chart-section-skeleton';
import { dashboardPanelCardClassName } from '@/components/dashboard/layout/constants';

afterEach(() => {
  cleanup();
});

describe('ChartSectionSkeleton (panel shell)', () => {
  it('renders a panel variant loading card with panel pointer events', () => {
    const { container } = render(<ChartSectionSkeleton />);

    const card = container.querySelector('[data-slot="card"]');
    expect(card).toBeTruthy();
    expect(card?.className).toContain('bg-surface-1');
    expect(card?.className).not.toContain('backdrop-blur');
    expect(card?.className).toContain(dashboardPanelCardClassName);
  });
});
