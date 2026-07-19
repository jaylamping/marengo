import { dashboardPanelPointerClassName } from '@/components/dashboard/layout/constants';

/** Re-enable pointer events on interactive simulation panels (Strategy C). */
export const simOverviewShellClassName = dashboardPanelPointerClassName;

/** Panel card variant for data-tier shells (tables, metrics, session). */
export const simDataShellVariant = 'panel' as const;

/** Panel card variant for hero-tier surfaces (viewport placeholder). */
export const simHeroShellVariant = 'panel' as const;

/** Chrome tier — opaque panel for the transport control bar. */
export const simControlBarClassName =
  'flex flex-wrap items-center gap-2 rounded-lg border border-line bg-surface-1 px-4 py-3 shadow-[0_8px_24px_rgb(0_0_0/0.3)] pointer-events-auto';

/** Data tier — opaque table rows inside a panel shell. */
export const simTableRowClassName = 'bg-surface-0 hover:bg-surface-1';

/** Data tier — opaque event log well inside a panel shell. */
export const simEventLogWellClassName =
  'max-h-48 space-y-2 overflow-y-auto rounded-md border border-line bg-surface-0 p-3 font-mono text-xs';

/** Panel card shell for session/metrics cards using DashboardCardShell. */
export const simDashboardCardShellClassName = dashboardPanelPointerClassName;
