/** Dummy battery pack metrics — replace with BMS telemetry later. */

export type BatteryPackRole = 'primary' | 'supplement';

export type BatteryPackStatus = 'discharging' | 'charging' | 'idle' | 'offline';

export type BatteryPack = {
  id: string;
  label: string;
  role: BatteryPackRole;
  location: string;
  socPercent: number;
  sohPercent: number;
  voltageV: number;
  capacityAh: number;
  tempC: number;
  status: BatteryPackStatus;
};

export type BatterySystemMetrics = {
  packs: BatteryPack[];
  estimatedRuntimeMin: number;
  detail: string;
};

export function computeAggregateSoc(packs: BatteryPack[]): number {
  const onlinePacks = packs.filter((pack) => pack.status !== 'offline');
  if (onlinePacks.length === 0) {
    return 0;
  }

  const totalCapacityAh = onlinePacks.reduce(
    (sum, pack) => sum + pack.capacityAh,
    0,
  );

  const weightedSoc = onlinePacks.reduce(
    (sum, pack) => sum + pack.socPercent * pack.capacityAh,
    0,
  );

  return Math.round(weightedSoc / totalCapacityAh);
}

export function getSystemStatus(
  packs: BatteryPack[],
): BatteryPackStatus | 'mixed' {
  const onlinePacks = packs.filter((pack) => pack.status !== 'offline');
  const statuses = new Set(onlinePacks.map((pack) => pack.status));

  if (statuses.size === 1) {
    return onlinePacks[0]?.status ?? 'idle';
  }

  return 'mixed';
}

export const dummyBatterySystemMetrics: BatterySystemMetrics = {
  packs: [
    {
      id: 'pelvis_main',
      label: 'Pelvis main',
      role: 'primary',
      location: 'pelvis',
      socPercent: 78,
      sohPercent: 96,
      voltageV: 47.2,
      capacityAh: 20,
      tempC: 31.4,
      status: 'discharging',
    },
    {
      id: 'supplement_a',
      label: 'Supplement A',
      role: 'supplement',
      location: 'torso',
      socPercent: 92,
      sohPercent: 98,
      voltageV: 47.5,
      capacityAh: 8,
      tempC: 29.8,
      status: 'discharging',
    },
    {
      id: 'supplement_b',
      label: 'Supplement B',
      role: 'supplement',
      location: 'backpack',
      socPercent: 65,
      sohPercent: 94,
      voltageV: 46.8,
      capacityAh: 8,
      tempC: 33.1,
      status: 'discharging',
    },
  ],
  estimatedRuntimeMin: 94,
  detail: '3 packs online · BMS wireframe · no cell faults',
};
