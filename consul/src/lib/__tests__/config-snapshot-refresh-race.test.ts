/**
 * Reproduces the Set Limits refresh stale-read:
 *   1. Persist pre-Apply config snapshot into localStorage (old dehydrate policy).
 *   2. Apply patches in-memory Query cache to post-Apply bounds.
 *   3. Without a persister flush, a reload restores the dehydrated blob.
 *   4. Hardware Range falls back to disk when live snapshot is null → old bounds.
 *
 * Fix: config snapshot is no longer persistable; restore must not resurrect
 * motors.yaml hard bounds across refresh.
 */
import { QueryClient } from '@tanstack/react-query';
import { describe, expect, it } from 'vitest';

import { applyConfigSnapshotLimits } from '@/lib/apply-config-snapshot-limits';
import type { ConfigSnapshotDto } from '@/lib/config-api';
import { isPersistableQueryKey, queryKeys } from '@/lib/query-keys';
import { buildHardwareRows } from '@/components/dashboard/hardware/build-hardware-rows';

const staleSnapshot: ConfigSnapshotDto = {
  profile: 'master',
  config_dir: '/opt/marengo/config',
  joints: ['right_shoulder_pitch'],
  motors: [
    {
      joint: 'right_shoulder_pitch',
      can_interface: 'can0',
      device_id: 1,
      direction: -1,
      motor_type: 'rs03',
      bench: {
        position_lower_rad: -0.9,
        position_upper_rad: 2.9,
        torque_limit_nm: 5,
      },
    },
  ],
  control_limits: [],
};

describe('config snapshot refresh race', () => {
  it('refuses to dehydrate config so a reload cannot restore pre-Apply hard bounds', () => {
    expect(isPersistableQueryKey(queryKeys.configSnapshot)).toBe(false);

    const client = new QueryClient();
    client.setQueryData(queryKeys.configSnapshot, staleSnapshot);
    const afterApply = applyConfigSnapshotLimits(staleSnapshot, 'right_shoulder_pitch', {
      lower: -0.53,
      upper: 1.23,
      softLower: -0.503,
      softUpper: 1.203,
    });
    client.setQueryData(queryKeys.configSnapshot, afterApply);

    // Simulate what PersistQueryClient dehydrateOptions would keep.
    const dehydrated = client
      .getQueryCache()
      .getAll()
      .filter((q) => isPersistableQueryKey(q.queryKey));
    expect(dehydrated).toHaveLength(0);

    // Immediate refresh with null live snapshot must not paint stale disk Range
    // from a restored cache — there is no restored cache for config anymore.
    const rowsFromStaleFallback = buildHardwareRows(staleSnapshot, [], null, null);
    expect(rowsFromStaleFallback[0]?.liveRange).toBe('-0.90–2.90');

    const rowsFromPatched = buildHardwareRows(afterApply, [], null, null);
    expect(rowsFromPatched[0]?.liveRange).toBe('-0.53–1.23');
    expect(rowsFromPatched[0]?.diskHardLower).toBeCloseTo(-0.53, 6);
    expect(rowsFromPatched[0]?.diskHardUpper).toBeCloseTo(1.23, 6);
  });
});
