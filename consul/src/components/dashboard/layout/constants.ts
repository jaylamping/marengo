import type { CSSProperties } from 'react';

export const dashboardLayoutStyle = {
  '--sidebar-width': 'calc(var(--spacing) * 72)',
  '--header-height': 'calc(var(--spacing) * 12)',
} as CSSProperties satisfies CSSProperties;

/** Strategy C: fullscreen R3F canvas host (z-0, non-interactive). */
export const sceneBackgroundClassName =
  'pointer-events-none fixed inset-0 z-0 will-change-transform';

/** Root shell for the 3-layer dashboard z-model. */
export const dashboardLayoutRootClassName = 'relative h-svh';

/** Chrome layer above the canvas (sidebar, header, inset). */
export const dashboardChromeClassName = 'relative z-20 min-h-svh bg-transparent';

/** Main route shell — re-enable pointer events on glass panels via children. */
export const dashboardMainPointerClassName = 'pointer-events-none';

/** Glass panels and interactive shells opt back into pointer events. */
export const dashboardPanelPointerClassName = 'pointer-events-auto';

export const dashboardMainClassName =
  '@container/main flex flex-1 flex-col gap-2';

export const urdfPreviewPanelClassName =
  'pointer-events-none min-h-[12rem] w-full bg-transparent';

export const dashboardOverviewClassName =
  'flex flex-col gap-4 py-4 md:gap-6 md:py-6';

export const dashboardLogsClassName =
  'flex min-h-0 flex-1 flex-col gap-4 px-4 py-4 lg:px-6';

export const dashboardChartSectionClassName = 'px-4 lg:px-6';
