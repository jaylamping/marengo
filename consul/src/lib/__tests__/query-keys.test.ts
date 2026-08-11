import { describe, expect, it } from 'vitest';

import { isPersistableQueryKey, queryKeys } from '@/lib/query-keys';

describe('isPersistableQueryKey', () => {
  it('does not persist config snapshot (Set Limits write-behind race)', () => {
    // ADR 0012: disk YAML is async write-behind. Persisting GET /config/snapshot in
    // localStorage (esp. with a 1s persister throttle) makes an immediate browser
    // refresh restore pre-Apply hard bounds until a later refetch/persist cycle.
    // Mutable limit SoT must come from live ActuatorLimitSnapshot + fresh network.
    expect(isPersistableQueryKey(queryKeys.configSnapshot)).toBe(false);
  });

  it('does not persist unrelated query keys', () => {
    expect(isPersistableQueryKey(queryKeys.hardwareCompleteness)).toBe(false);
    expect(isPersistableQueryKey(queryKeys.commissioningScope)).toBe(false);
  });
});
