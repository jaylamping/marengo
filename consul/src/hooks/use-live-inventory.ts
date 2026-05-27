import { useMemo } from 'react';

import type { InventoryItem } from '@/data/robot-inventory';
import { isChappeLive } from '@/lib/chappe-config';
import { useRobotStore } from '@/state/robotStore';

/** Overlay live joint positions from Chappe onto static inventory rows. */
export function useLiveInventory(base: InventoryItem[]): InventoryItem[] {
  const robotState = useRobotStore((s) => s.robotState);
  const imuSample = useRobotStore((s) => s.imuSample);
  const connected = useRobotStore((s) => s.connected);

  return useMemo(() => {
    if (!isChappeLive() || !connected) {
      return base;
    }
    const byName = new Map(
      robotState?.joints.map((j) => [j.name, j.position] as const) ?? [],
    );
    return base.map((row) => {
      if (row.name === 'torso_imu' && imuSample) {
        return {
          ...row,
          value: `q=${imuSample.quaternionReal.toFixed(3)}`,
          status: 'Nominal' as const,
        };
      }
      const pos = byName.get(row.name);
      if (pos === undefined || row.kind !== 'actuator') {
        return row;
      }
      return {
        ...row,
        value: pos.toFixed(2),
        status: row.status === 'Offline' ? 'Enabled' : row.status,
      };
    });
  }, [base, connected, imuSample, robotState]);
}
