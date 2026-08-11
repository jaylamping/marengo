export type SidebarIconKey =
  | 'overview'
  | 'simulation'
  | 'actuators'
  | 'visualizer'
  | 'subsystems'
  | 'hardware'
  | 'safety'
  | 'telemetry'
  | 'logs'
  | 'settings'
  | 'docs'
  | 'search'
  | 'preset-golden'
  | 'preset-bench'
  | 'preset-tuning';

export type SidebarNavItem = {
  title: string;
  url: string;
  icon: SidebarIconKey;
};

export type SidebarPresetItem = {
  name: string;
  url: string;
  icon: SidebarIconKey;
};

export type SidebarUser = {
  name: string;
  /** Bench context under the name — `local` (Vite dev) or `live` (Pi-hosted build). */
  context: string;
  avatar: string;
};

/** `local` under Vite dev; `live` for production builds served from the Pi. */
export function sidebarContextLabel(isProd: boolean = import.meta.env.PROD): string {
  return isProd ? 'live' : 'local';
}

/** Operator identity; context flips with Vite production vs development builds. */
export function getSidebarUser(): SidebarUser {
  return {
    name: 'Joey',
    context: sidebarContextLabel(),
    avatar: '',
  };
}

/** Live routes only — stubs stay out of the nav until they ship. */
const sidebarNavMainBase: SidebarNavItem[] = [
  { title: 'Overview', url: '/', icon: 'overview' },
  { title: 'Simulation', url: '/simulation', icon: 'simulation' },
  { title: 'Telemetry', url: '/telemetry', icon: 'telemetry' },
  { title: 'Hardware', url: '/hardware', icon: 'hardware' },
  { title: 'Testing', url: '/testing', icon: 'preset-tuning' },
  { title: 'Logs', url: '/logs', icon: 'logs' },
];

/** Main nav — Telemetry replaces Subsystems; Actuators retired. */
export function getSidebarNavMain(): SidebarNavItem[] {
  return sidebarNavMainBase;
}

/** @deprecated Use getSidebarNavMain(). */
export const sidebarNavMain: SidebarNavItem[] = sidebarNavMainBase;

/** Reserved — empty until Settings/Docs/Search are real routes. */
export const sidebarNavSecondary: SidebarNavItem[] = [];

/** Reserved — empty until pose presets apply real holds. */
export const sidebarPresets: SidebarPresetItem[] = [];
