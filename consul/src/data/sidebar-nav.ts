export type SidebarIconKey =
  | 'overview'
  | 'simulation'
  | 'visualizer'
  | 'joints'
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

export const sidebarNavMain: SidebarNavItem[] = [
  { title: 'Overview', url: '/', icon: 'overview' },
  { title: 'Simulation', url: '/simulation', icon: 'simulation' },
  { title: 'Visualizer', url: '#', icon: 'visualizer' },
  { title: 'Joints', url: '#', icon: 'joints' },
  { title: 'Safety', url: '#', icon: 'safety' },
  { title: 'Telemetry', url: '#', icon: 'telemetry' },
  { title: 'Logs', url: '/logs', icon: 'logs' },
];

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
