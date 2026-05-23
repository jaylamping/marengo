import type { ChartConfig } from '@/components/ui/chart';

import type { InventoryView } from '@/components/dashboard/inventory/types';

export const INVENTORY_VIEW_OPTIONS: Array<{
  label: string;
  value: InventoryView;
}> = [
  { label: 'All Devices', value: 'all' },
  { label: 'Faults', value: 'faults' },
  { label: 'Offline', value: 'offline' },
  { label: 'Unconfigured', value: 'unconfigured' },
];

export const PRESET_OPTIONS = [
  { label: 'golden_pose', value: 'golden_pose' },
  { label: 'bench_default', value: 'bench_default' },
  { label: 'tuning_sweep', value: 'tuning_sweep' },
  { label: 'last_session', value: 'last_session' },
] as const;

export const PRESET_OPTIONS_WITH_UNASSIGNED = [
  ...PRESET_OPTIONS,
  { label: 'unassigned', value: 'unassigned' },
] as const;

export const KIND_OPTIONS = [
  { label: 'actuator', value: 'actuator' },
  { label: 'sensor', value: 'sensor' },
  { label: 'device', value: 'device' },
] as const;

export const STATUS_OPTIONS = [
  { label: 'Enabled', value: 'Enabled' },
  { label: 'Nominal', value: 'Nominal' },
  { label: 'Tuning', value: 'Tuning' },
  { label: 'Fault', value: 'Fault' },
  { label: 'Offline', value: 'Offline' },
] as const;

export const actuatorTrackingChartData = [
  { sample: '0s', commanded: 0.42, measured: 0.38 },
  { sample: '10s', commanded: 0.61, measured: 0.58 },
  { sample: '20s', commanded: 0.72, measured: 0.69 },
  { sample: '30s', commanded: 0.58, measured: 0.55 },
  { sample: '40s', commanded: 0.74, measured: 0.71 },
  { sample: '50s', commanded: 0.85, measured: 0.79 },
  { sample: '60s', commanded: 0.78, measured: 0.73 },
];

export const actuatorTrackingChartConfig = {
  commanded: {
    label: 'Commanded (Nm)',
    color: 'var(--primary)',
  },
  measured: {
    label: 'Measured (Nm)',
    color: 'var(--chart-2)',
  },
} satisfies ChartConfig;
