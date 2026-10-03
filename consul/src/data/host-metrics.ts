/** Dummy host metrics — replace with Chappe telemetry later. */

export type PiHostMetrics = {
  hostname: string;
  cpuPercent: number | undefined;
  ramUsedGb: number;
  ramTotalGb: number;
  diskUsedGb: number | null;
  diskTotalGb: number | null;
  logDiskUsedGb: number | null;
  logDiskBudgetGb: number | null;
  tempC: number;
  load1m: number;
  uptime: string;
  throttled: boolean;
};

export const dummyPiHostMetrics: PiHostMetrics = {
  hostname: 'marengo-pi',
  cpuPercent: 24,
  ramUsedGb: 2.1,
  ramTotalGb: 8,
  diskUsedGb: 12,
  diskTotalGb: 58,
  logDiskUsedGb: 0.82,
  logDiskBudgetGb: 5,
  tempC: 52.3,
  load1m: 0.42,
  uptime: '4h 12m',
  throttled: false,
};

