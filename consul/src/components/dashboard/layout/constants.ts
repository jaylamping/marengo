import type { CSSProperties } from 'react';

export const dashboardLayoutStyle = {
  '--sidebar-width': 'calc(var(--spacing) * 72)',
  '--header-height': 'calc(var(--spacing) * 12)',
} as CSSProperties satisfies CSSProperties;

export const dashboardMainClassName =
  '@container/main flex flex-1 flex-col gap-2';

export const dashboardOverviewClassName =
  'flex flex-col gap-4 py-4 md:gap-6 md:py-6';

export const dashboardChartSectionClassName = 'px-4 lg:px-6';
