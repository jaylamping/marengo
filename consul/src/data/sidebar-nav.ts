import { isActuatorsFeatureEnabled } from '@/lib/feature-flags';

export type SidebarIconKey =
  | 'overview'
  | 'simulation'
  | 'actuators'
  | 'visualizer'
  | 'subsystems'
  | 'safety'
  | 'telemetry'
  | 'logs'
  | 'memory'
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

const sidebarNavMainBase: SidebarNavItem[] = [
  { title: 'Overview', url: '/', icon: 'overview' },
  { title: 'Simulation', url: '/simulation', icon: 'simulation' },
  { title: 'Visualizer', url: '#', icon: 'visualizer' },
  { title: 'Subsystems', url: '/subsystems', icon: 'subsystems' },
  { title: 'Safety', url: '#', icon: 'safety' },
  { title: 'Telemetry', url: '#', icon: 'telemetry' },
  { title: 'Logs', url: '/logs', icon: 'logs' },
  { title: 'Memory', url: '/memory', icon: 'memory' },
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

export const sidebarNavSecondary: SidebarNavItem[] = [
  { title: 'Settings', url: '#', icon: 'settings' },
  { title: 'Docs', url: '#', icon: 'docs' },
  { title: 'Search', url: '#', icon: 'search' },
];

export const sidebarPresets: SidebarPresetItem[] = [
  { name: 'golden_pose', url: '#', icon: 'preset-golden' },
  { name: 'bench_default', url: '#', icon: 'preset-bench' },
  { name: 'tuning_sweep', url: '#', icon: 'preset-tuning' },
];
