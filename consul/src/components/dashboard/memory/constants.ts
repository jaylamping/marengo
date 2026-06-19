import { dashboardPanelPointerClassName } from '@/components/dashboard/layout/constants';

const memoryGlassSurfaceClassName = [
  'border border-white/20 [border-top-color:var(--glass-refraction-top)]',
  'bg-[radial-gradient(ellipse_at_50%_0%,rgb(255_255_255_/_0.16),transparent_50%),linear-gradient(to_bottom,rgb(255_255_255_/_0.1),rgb(255_255_255_/_0.04))]',
  'backdrop-blur-xl backdrop-saturate-[180%]',
  'shadow-[0_0_0_1px_rgb(255_255_255_/_0.1)_inset,var(--shadow-glass-sm)]',
  'dark:border-white/[0.1]',
  'dark:bg-[radial-gradient(ellipse_at_50%_0%,rgb(255_255_255_/_0.05),transparent_50%),linear-gradient(to_bottom,rgb(255_255_255_/_0.03),rgb(255_255_255_/_0.01))]',
  'dark:shadow-[0_0_0_1px_rgb(255_255_255_/_0.05)_inset,0_8px_24px_rgb(0_0_0_/_0.35)]',
].join(' ');

export const memorySearchPanelShellClassName = [
  'flex flex-wrap items-center gap-2 rounded-lg px-4 py-3',
  dashboardPanelPointerClassName,
  memoryGlassSurfaceClassName,
].join(' ');

export const memoryListShellClassName = [
  'h-[420px] overflow-auto rounded-lg',
  dashboardPanelPointerClassName,
  memoryGlassSurfaceClassName,
].join(' ');

export const memorySheetContentClassName = [
  'border border-white/20 [border-top-color:var(--glass-refraction-top)]',
  'bg-[radial-gradient(ellipse_at_50%_0%,rgb(255_255_255_/_0.12),transparent_50%),linear-gradient(to_bottom,rgb(255_255_255_/_0.08),rgb(255_255_255_/_0.03))]',
  'backdrop-blur-xl backdrop-saturate-[180%]',
  'shadow-[0_0_0_1px_rgb(255_255_255_/_0.1)_inset,var(--shadow-glass-sm)]',
  'dark:border-white/[0.1]',
  'dark:bg-[radial-gradient(ellipse_at_50%_0%,rgb(255_255_255_/_0.04),transparent_50%),linear-gradient(to_bottom,rgb(255_255_255_/_0.02),rgb(255_255_255_/_0.01))]',
].join(' ');

export const memoryTabsVariant = 'glass' as const;
