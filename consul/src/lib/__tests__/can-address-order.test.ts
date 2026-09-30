import { describe, expect, it } from 'vitest';

import {
  compareCanAddress,
  parseInventoryNodeCanAddress,
} from '@/lib/can-address-order';

describe('parseInventoryNodeCanAddress', () => {
  it('parses motor node labels', () => {
    expect(parseInventoryNodeCanAddress('RS03 · can0 · id 2')).toEqual({
      canInterface: 'can0',
      canId: 2,
    });
  });

  it('returns nulls for non-CAN nodes', () => {
    expect(parseInventoryNodeCanAddress('configuration unavailable')).toEqual({
      canInterface: null,
      canId: null,
    });
    expect(parseInventoryNodeCanAddress('i2c-1 · 0x4b · BNO085')).toEqual({
      canInterface: null,
      canId: null,
    });
  });
});

describe('compareCanAddress', () => {
  it('orders by interface then id', () => {
    const rows = [
      { joint: 'b', canInterface: 'can0', canId: 2 },
      { joint: 'offline', canInterface: null, canId: null },
      { joint: 'a', canInterface: 'can0', canId: 1 },
      { joint: 'can1-first', canInterface: 'can1', canId: 1 },
    ];
    rows.sort(compareCanAddress);
    expect(rows.map((r) => r.joint)).toEqual([
      'a',
      'b',
      'can1-first',
      'offline',
    ]);
  });
});
