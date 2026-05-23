/** Dummy host + control metrics — replace with Chappe telemetry later. */

export type PiHostMetrics = {
  hostname: string;
  cpuPercent: number;
  ramUsedGb: number;
  ramTotalGb: number;
  tempC: number;
  load1m: number;
  uptime: string;
  throttled: boolean;
  servicesLabel: string;
};

export type JetsonHostMetrics = {
  hostname: string;
  cpuPercent: number;
  ramUsedGb: number;
  ramTotalGb: number;
  gpuPercent: number;
  tempC: number;
  load1m: number;
  uptime: string;
  powerMode: string;
  chappeRttMs: number;
  online: boolean;
  servicesLabel: string;
};

export type CanBusMetrics = {
  interface: string;
  status: string;
  nodeCount: number;
  avgRttMs: number;
  detail: string;
};

export type ControlLoopMetrics = {
  rateHz: number;
  p99Ms: number;
  summary: string;
  detail: string;
};

export const dummyPiHostMetrics: PiHostMetrics = {
  hostname: 'marengo-pi',
  cpuPercent: 24,
  ramUsedGb: 2.1,
  ramTotalGb: 8,
  tempC: 52.3,
  load1m: 0.42,
  uptime: '4h 12m',
  throttled: false,
  servicesLabel: 'Consul · Berthier · Davout on same host',
};

export const dummyJetsonHostMetrics: JetsonHostMetrics = {
  hostname: 'marengo-jetson',
  cpuPercent: 38,
  ramUsedGb: 5.4,
  ramTotalGb: 16,
  gpuPercent: 61,
  tempC: 48.7,
  load1m: 1.12,
  uptime: '4h 12m',
  powerMode: 'MAXN',
  chappeRttMs: 1.4,
  online: true,
  servicesLabel: 'Fouché · planner · Chappe to Pi',
};

export const dummyCanBusMetrics: CanBusMetrics = {
  interface: 'can0',
  status: 'OK',
  nodeCount: 4,
  avgRttMs: 0.8,
  detail: 'RS03×2 · RS02×2 · no missed heartbeats',
};

export const dummyControlLoopMetrics: ControlLoopMetrics = {
  rateHz: 500,
  p99Ms: 1.2,
  summary: 'Berthier → Davout steady',
  detail: 'Last overruns: none this session',
};
