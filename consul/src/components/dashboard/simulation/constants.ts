import { dashboardPanelPointerClassName } from '@/components/dashboard/layout/constants';

/** Re-enable pointer events on interactive simulation panels (Strategy C). */
export const simOverviewShellClassName = dashboardPanelPointerClassName;

/** Glass card variant for data-tier shells (tables, metrics, session). */
export const simDataShellVariant = 'glass' as const;

/** Glass card variant for hero-tier surfaces (viewport placeholder). */
export const simHeroShellVariant = 'glass' as const;

/** Chrome tier — full blur + border for the transport control bar. */
export const simControlBarClassName =
  'flex flex-wrap items-center gap-2 rounded-lg border border-white/20 [border-top-color:var(--glass-refraction-top)] bg-[var(--glass-2-surface)] px-4 py-3 backdrop-blur-xl backdrop-saturate-[180%] shadow-[0_0_0_1px_rgb(255_255_255_/_0.1)_inset,var(--shadow-glass-sm)] pointer-events-auto';

/** Data tier — opaque table rows inside a glass shell. */
export const simTableRowClassName = 'bg-card/95 hover:bg-card';

/** Data tier — opaque event log well inside a glass shell. */
export const simEventLogWellClassName =
  'max-h-48 space-y-2 overflow-y-auto rounded-md border bg-card/95 p-3 font-mono text-xs';

/** Glass card shell for session/metrics cards using DashboardCardShell. */
export const simDashboardCardShellClassName = dashboardPanelPointerClassName;
