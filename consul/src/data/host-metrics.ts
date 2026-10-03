/** Dummy host metrics — replace with Chappe telemetry later. */

export type PiHostMetrics = {
  hostname: string;
  cpuPercent: number | undefined;
  /** Null when /proc/meminfo was unreadable: unknown, never an empty host. */
  ramUsedGb: number | null;
  ramTotalGb: number | null;
  diskUsedGb: number | null;
  diskTotalGb: number | null;
  logDiskUsedGb: number | null;
  logDiskBudgetGb: number | null;
  /** Null when no thermal zone was readable: unknown, never 0 °C. */
  tempC: number | null;
  load1m: number;
  uptime: string;
  /** Null when vcgencmd was unavailable: unknown, never "not throttled". */
  throttled: boolean | null;
  /** True when the producer is the non-Linux dev stub (synthetic data). */
  simulated: boolean;
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
  simulated: false,
};

