import { dashboardPanelPointerClassName } from '@/components/dashboard/layout/constants';

/** Maps sidebar semantic tokens onto opaque panel surfaces. */
export const sidebarPanelSkinStyle = {
  '--sidebar': 'var(--surface-1)',
  '--sidebar-border': 'var(--line)',
  '--sidebar-foreground': 'var(--foreground)',
} as const;

/** Opaque panel surface applied to the visible sidebar shell only. */
export const sidebarPanelSkinClassName = [
  'border border-line bg-surface-1',
  'shadow-[0_8px_24px_rgb(0_0_0/0.3)]',
].join(' ');

/** Strategy C chrome: sidebar above canvas with pointer events restored. */
export const sidebarChromeClassName = ['z-30', dashboardPanelPointerClassName].join(
  ' ',
);

/** Opaque panel shell for the dashboard site header (sticky within inset scroll). */
export const siteHeaderPanelClassName = [
  'sticky top-0 z-30',
  dashboardPanelPointerClassName,
  'border-b border-line bg-surface-1',
].join(' ');
