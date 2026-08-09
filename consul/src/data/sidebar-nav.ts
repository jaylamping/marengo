import { isActuatorsFeatureEnabled } from '@/lib/feature-flags';

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
  email: string;
  avatar: string;
};

export const sidebarUser: SidebarUser = {
  name: 'Joey',
  email: 'local · can0',
  avatar: '',
};

/** Live routes only — stubs stay out of the nav until they ship. */
const sidebarNavMainBase: SidebarNavItem[] = [
  { title: 'Overview', url: '/', icon: 'overview' },
  { title: 'Simulation', url: '/simulation', icon: 'simulation' },
  { title: 'Subsystems', url: '/subsystems', icon: 'subsystems' },
  { title: 'Hardware', url: '/hardware', icon: 'hardware' },
  { title: 'Testing', url: '/testing', icon: 'preset-tuning' },
  { title: 'Logs', url: '/logs', icon: 'logs' },
];

const actuatorsNavItem: SidebarNavItem = {
  title: 'Actuators',
  url: '/actuators',
  icon: 'actuators',
};

/** Main nav with feature-gated Actuators entry after Simulation. */
export function getSidebarNavMain(): SidebarNavItem[] {
  if (!isActuatorsFeatureEnabled()) {
    return sidebarNavMainBase;
  }
  const [overview, simulation, ...rest] = sidebarNavMainBase;
  return [overview, simulation, actuatorsNavItem, ...rest];
}

/** @deprecated Use getSidebarNavMain() for feature-aware navigation. */
export const sidebarNavMain: SidebarNavItem[] = sidebarNavMainBase;

/** Reserved — empty until Settings/Docs/Search are real routes. */
export const sidebarNavSecondary: SidebarNavItem[] = [];

/** Reserved — empty until pose presets apply real holds. */
export const sidebarPresets: SidebarPresetItem[] = [];
