import { useMemo } from 'react';

import type { InventoryItem, InventoryStatus } from '@/data/robot-inventory';
import { ImuAccuracy, type JointState } from '@/gen/marengo/v1/marengo_pb';
import { isChappeLive } from '@/lib/chappe-config';
import { useRobotStore } from '@/state/robotStore';

/** A joint's status from its own wire fields: fault bits, then drive_active. */
export function jointInventoryStatus(joint: JointState): InventoryStatus {
  if (joint.fault !== 0) {
    return 'Fault';
  }
  return joint.driveActive ? 'Enabled' : 'Disabled';
}

/**
 * Overlay live Chappe state onto inventory rows. A row with no live sample
 * keeps the catalog's Offline / '—': nothing is shown that the wire did not say.
 */
export function useLiveInventory(base: InventoryItem[]): InventoryItem[] {
  const robotState = useRobotStore((s) => s.robotState);
  const imuSample = useRobotStore((s) => s.imuSample);
  const connected = useRobotStore((s) => s.connected);

  return useMemo(() => {
    if (!isChappeLive() || !connected) {
      return base;
    }
    const byName = new Map(
      robotState?.joints.map((j) => [j.name, j] as const) ?? [],
    );
    return base.map((row) => {
      if (row.name === 'torso_imu' && imuSample) {
        return {
          ...row,
          value: `q=${imuSample.quaternionReal.toFixed(3)} · acc ${ImuAccuracy[imuSample.accuracy]}`,
          status: 'Nominal' as const,
        };
      }
      const joint = byName.get(row.name);
      if (joint === undefined || row.kind !== 'actuator') {
        return row;
      }
      return {
        ...row,
        value: joint.position.toFixed(2),
        status: jointInventoryStatus(joint),
      };
    });
  }, [base, connected, imuSample, robotState]);
}
