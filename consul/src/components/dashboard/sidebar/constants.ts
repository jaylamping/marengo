import { dashboardPanelPointerClassName } from '@/components/dashboard/layout/constants';

/** Maps sidebar semantic tokens onto glass surfaces (single blur layer). */
export const sidebarGlassSkinStyle = {
  '--sidebar': 'var(--glass-2-surface)',
  '--sidebar-border': 'var(--glass-border)',
} as const;

/** Glass panel surface applied to the visible sidebar shell only. */
export const sidebarGlassSkinClassName = [
  'border border-white/20 [border-top-color:var(--glass-refraction-top)]',
  'bg-[radial-gradient(ellipse_at_50%_0%,rgb(255_255_255_/_0.16),transparent_50%),linear-gradient(to_bottom,rgb(255_255_255_/_0.1),rgb(255_255_255_/_0.04))]',
  'backdrop-blur-xl backdrop-saturate-[180%]',
  'shadow-[0_0_0_1px_rgb(255_255_255_/_0.1)_inset,var(--shadow-glass-sm)]',
  'dark:border-white/[0.1]',
  'dark:bg-[radial-gradient(ellipse_at_50%_0%,rgb(255_255_255_/_0.05),transparent_50%),linear-gradient(to_bottom,rgb(255_255_255_/_0.03),rgb(255_255_255_/_0.01))]',
].join(' ');

/** Strategy C chrome: sidebar above canvas with pointer events restored. */
export const sidebarChromeClassName = ['z-30', dashboardPanelPointerClassName].join(
  ' ',
);

/** Glass shell for the dashboard site header. */
export const siteHeaderGlassClassName = [
  'relative z-30',
  dashboardPanelPointerClassName,
  'border-b border-white/20 [border-top-color:var(--glass-refraction-top)]',
  'bg-[var(--glass-2-surface)] backdrop-blur-xl backdrop-saturate-[180%]',
  'shadow-[0_0_0_1px_rgb(255_255_255_/_0.1)_inset,var(--shadow-glass-sm)]',
  'dark:border-white/[0.1]',
].join(' ');
