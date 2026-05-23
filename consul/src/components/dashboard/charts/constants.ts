import type { ChartConfig } from '@/components/ui/chart';

import type { ChartTimeRange, JointTrackingSeries } from '@/components/dashboard/charts/types';

export const CHART_TIME_RANGE_OPTIONS: Array<{
  value: ChartTimeRange;
  label: string;
}> = [
  { value: 'session', label: 'Session' },
  { value: '5m', label: 'Last 5 min' },
  { value: '1m', label: 'Last 1 min' },
];

export const dummyShoulderPitchTracking: JointTrackingSeries = {
  jointName: 'shoulder_pitch',
  title: 'shoulder_pitch · torque tracking',
  description: 'Commanded vs measured torque for current session',
  descriptionShort: 'Torque tracking',
  points: [
    { time: '00:00', commanded: 0.42, measured: 0.38 },
    { time: '00:05', commanded: 0.55, measured: 0.51 },
    { time: '00:10', commanded: 0.61, measured: 0.58 },
    { time: '00:15', commanded: 0.48, measured: 0.44 },
    { time: '00:20', commanded: 0.72, measured: 0.69 },
    { time: '00:25', commanded: 0.66, measured: 0.62 },
    { time: '00:30', commanded: 0.58, measured: 0.55 },
    { time: '00:35', commanded: 0.81, measured: 0.77 },
    { time: '00:40', commanded: 0.74, measured: 0.71 },
    { time: '00:45', commanded: 0.69, measured: 0.64 },
    { time: '00:50', commanded: 0.92, measured: 0.88 },
    { time: '00:55', commanded: 0.85, measured: 0.79 },
    { time: '01:00', commanded: 0.78, measured: 0.73 },
    { time: '01:05', commanded: 0.63, measured: 0.59 },
    { time: '01:10', commanded: 0.57, measured: 0.52 },
    { time: '01:15', commanded: 0.49, measured: 0.45 },
    { time: '01:20', commanded: 0.71, measured: 0.67 },
    { time: '01:25', commanded: 0.88, measured: 0.84 },
    { time: '01:30', commanded: 0.94, measured: 0.89 },
    { time: '01:35', commanded: 0.86, measured: 0.81 },
    { time: '01:40', commanded: 0.79, measured: 0.74 },
    { time: '01:45', commanded: 0.68, measured: 0.63 },
    { time: '01:50', commanded: 0.59, measured: 0.54 },
    { time: '01:55', commanded: 0.52, measured: 0.48 },
    { time: '02:00', commanded: 0.47, measured: 0.43 },
  ],
};

export const jointTrackingChartConfig = {
  tracking: {
    label: 'Tracking',
  },
  commanded: {
    label: 'Commanded (Nm)',
    color: 'var(--primary)',
  },
  measured: {
    label: 'Measured (Nm)',
    color: 'var(--chart-2)',
  },
} satisfies ChartConfig;
