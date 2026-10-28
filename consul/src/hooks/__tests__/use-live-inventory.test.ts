// @vitest-environment jsdom
import { renderHook } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';

import { robotInventory } from '@/data/robot-inventory';
import { useLiveInventory } from '@/hooks/use-live-inventory';
import { applyOverrides } from '@/state/inventoryOverridesStore';
import { useRobotStore } from '@/state/robotStore';

vi.mock('@/lib/chappe-config', () => ({ isChappeLive: () => true }));

afterEach(() => {
  useRobotStore.setState({ connected: false, robotState: null, imuSample: null });
});

function row(name: string, rows: typeof robotInventory) {
  const found = rows.find((item) => item.name === name);
  if (!found) throw new Error(`no row ${name}`);
  return found;
}

describe('live inventory status (UI-08)', () => {
  it('disconnected, no row claims a state the wire did not report', () => {
    const { result } = renderHook(() => useLiveInventory(robotInventory));
    for (const item of result.current) {
      expect(item.status).toBe('Offline');
      expect(item.value).toBe('—');
    }
  });

  it('connected, a joint reads its own drive_active and fault bits', () => {
    useRobotStore.setState({
      connected: true,
      robotState: {
        joints: [
          { name: 'right_elbow_pitch', position: 0.1, driveActive: false, fault: 0 },
          { name: 'right_shoulder_pitch', position: 0.2, driveActive: true, fault: 0 },
          { name: 'right_shoulder_roll', position: 0.3, driveActive: true, fault: 0x4 },
        ],
      } as never,
    });
    const { result } = renderHook(() => useLiveInventory(robotInventory));
    expect(row('right_elbow_pitch', result.current).status).toBe('Disabled');
    expect(row('right_shoulder_pitch', result.current).status).toBe('Enabled');
    expect(row('right_shoulder_roll', result.current).status).toBe('Fault');
    // No sample: stays Offline, not Enabled from the catalog.
    expect(row('right_lower_arm_yaw', result.current).status).toBe('Offline');
  });

  it('a stored identity patch never overrides the live status', () => {
    const item = row('right_elbow_pitch', robotInventory);
    expect(applyOverrides(item, { [item.id]: { status: 'Enabled' } }).status).toBe('Offline');
  });
});
