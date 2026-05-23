/** Dummy Isaac Sim session data — wireframe until D2 Lab bridge lands. */

export type SimSessionState = 'disconnected' | 'idle' | 'running' | 'paused';

export type SimScenarioStatus = 'ready' | 'running' | 'passed' | 'failed';

export type SimScenario = {
  id: string;
  name: string;
  world: string;
  description: string;
  status: SimScenarioStatus;
  lastRun: string;
};

export type SimEvent = {
  id: string;
  timestamp: string;
  level: 'info' | 'warn' | 'error';
  message: string;
};

export type SimRuntimeMetrics = {
  simTimeS: number;
  realTimeFactor: number;
  physicsDtMs: number;
  renderFps: number;
  gpuUtilPercent: number;
  gpuMemUsedGb: number;
  gpuMemTotalGb: number;
};

export type SimSession = {
  state: SimSessionState;
  host: string;
  kitVersion: string;
  world: string;
  robotAsset: string;
  extension: string;
};

export const dummySimSession: SimSession = {
  state: 'idle',
  host: 'jetson · localhost:8211',
  kitVersion: 'Isaac Sim 4.5.0',
  world: 'marengo_bench_cell.usd',
  robotAsset: 'marengo_arm_4dof.usd',
  extension: 'marengo.lab.tasks',
};

export const dummySimRuntimeMetrics: SimRuntimeMetrics = {
  simTimeS: 0,
  realTimeFactor: 0,
  physicsDtMs: 16.67,
  renderFps: 0,
  gpuUtilPercent: 0,
  gpuMemUsedGb: 0,
  gpuMemTotalGb: 16,
};

export const dummySimScenarios: SimScenario[] = [
  {
    id: 'bench_home',
    name: 'bench_home',
    world: 'marengo_bench_cell',
    description: 'Arm to golden pose, hold 5 s, check joint error',
    status: 'ready',
    lastRun: '—',
  },
  {
    id: 'pick_mug',
    name: 'pick_mug_smoke',
    world: 'warehouse_flat',
    description: 'Pick-place mug with scripted gripper sequence',
    status: 'ready',
    lastRun: '—',
  },
  {
    id: 'loco_smoke',
    name: 'locomotion_smoke',
    world: 'flat_ground',
    description: 'Standing balance + 3-step walk (23-DOF target)',
    status: 'ready',
    lastRun: '—',
  },
];

export const dummySimEvents: SimEvent[] = [
  {
    id: '1',
    timestamp: '—',
    level: 'info',
    message: 'Wireframe session — connect Isaac Sim to populate events.',
  },
];
