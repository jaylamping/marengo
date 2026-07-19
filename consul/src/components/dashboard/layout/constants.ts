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

/**
 * Chrome layer above the canvas (sidebar, header, inset).
 * Must override shadcn’s `has-data-[variant=inset]:bg-sidebar` — that opaque
 * fill only applies on desktop (mobile Sheet never sets data-variant), which
 * is why the ambient backdrop vanished above the md breakpoint.
 */
export const dashboardChromeClassName =
  'relative z-20 min-h-svh bg-transparent has-data-[variant=inset]:bg-transparent';

/** Main route shell — re-enable pointer events on panels via children. */
export const dashboardMainPointerClassName = 'pointer-events-none';

/** Panels and interactive shells opt back into pointer events. */
export const dashboardPanelPointerClassName = 'pointer-events-auto';

/** Shared panel card shell — variant=panel + Strategy C pointer threading. */
export const dashboardPanelCardClassName = dashboardPanelPointerClassName;

/** Overview metric card grid (panel cards supply their own surface). */
export const sectionCardsGridClassName =
  'grid grid-cols-1 gap-4 px-4 lg:px-6 @xl/main:grid-cols-2 @5xl/main:grid-cols-3';

export const dashboardMainClassName =
  '@container/main flex flex-1 flex-col gap-2';

export const urdfPreviewPanelClassName =
  'pointer-events-none min-h-[12rem] w-full bg-transparent';

export const dashboardOverviewClassName =
  'flex flex-col gap-4 py-4 md:gap-6 md:py-6';

export const dashboardLogsClassName =
  'flex min-h-0 flex-1 flex-col gap-4 px-4 py-4 lg:px-6';

export const dashboardSubsystemsClassName = dashboardLogsClassName;

export const dashboardChartSectionClassName = 'px-4 lg:px-6';
