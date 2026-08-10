// @vitest-environment jsdom
import { beforeEach, describe, expect, it } from 'vitest';

import {
  INVENTORY_OVERRIDES_STORAGE_KEY,
  parseInventoryOverridesPersisted,
  useInventoryOverridesStore,
} from '@/state/inventoryOverridesStore';

describe('parseInventoryOverridesPersisted', () => {
  it('returns empty overrides for corrupt or unsupported persisted data', () => {
    expect(parseInventoryOverridesPersisted('{not-json')).toEqual({
      version: 1,
      overrides: {},
    });
    expect(parseInventoryOverridesPersisted(JSON.stringify({ version: 2, overrides: {} }))).toEqual({
      version: 1,
      overrides: {},
    });
  });

  it('keeps valid fields and drops unknown group/kind/status enums', () => {
    expect(
      parseInventoryOverridesPersisted(
        JSON.stringify({
          version: 1,
          overrides: {
            27: {
              preset: 'unassigned',
              group: 'not_a_group',
              kind: 'actuator',
              status: 'Hacked',
              name: 'yaw',
            },
          },
        }),
      ),
    ).toEqual({
      version: 1,
      overrides: {
        27: { preset: 'unassigned', kind: 'actuator', name: 'yaw' },
      },
    });
  });
});

describe('useInventoryOverridesStore', () => {
  beforeEach(() => {
    localStorage.clear();
    useInventoryOverridesStore.setState({ overrides: {} });
  });

  it('merges patches and round-trips them through localStorage', () => {
    useInventoryOverridesStore.getState().applyPatch(27, { preset: 'bench_3dof' });
    useInventoryOverridesStore.getState().applyPatch(27, { name: 'right_yaw' });

    expect(useInventoryOverridesStore.getState().overrides).toEqual({
      27: { preset: 'bench_3dof', name: 'right_yaw' },
    });
    expect(
      parseInventoryOverridesPersisted(
        localStorage.getItem(INVENTORY_OVERRIDES_STORAGE_KEY),
      ),
    ).toEqual({
      version: 1,
      overrides: { 27: { preset: 'bench_3dof', name: 'right_yaw' } },
    });
  });
});
