/** Shared CAN bus + device-id ordering for Hardware and inventory tables. */

export type CanAddressSortKey = {
  canInterface: string | null;
  canId: number | null;
  joint: string;
};

/** Bus name, then device id; unwired (null id) sorts last; joint name tiebreak. */
export function compareCanAddress(a: CanAddressSortKey, b: CanAddressSortKey): number {
  const ifaceA = a.canInterface ?? '\uffff';
  const ifaceB = b.canInterface ?? '\uffff';
  const ifaceCmp = ifaceA.localeCompare(ifaceB);
  if (ifaceCmp !== 0) return ifaceCmp;

  const idA = a.canId ?? Number.POSITIVE_INFINITY;
  const idB = b.canId ?? Number.POSITIVE_INFINITY;
  if (idA !== idB) return idA - idB;

  return a.joint.localeCompare(b.joint);
}

/**
 * Parse Consul inventory `node` strings like `RS03 · can0 · id 2`.
 * Non-CAN nodes (i2c / gpio / unavailable) return nulls.
 */
export function parseInventoryNodeCanAddress(node: string): {
  canInterface: string | null;
  canId: number | null;
} {
  const match = /^(.+?) · (.+?) · id (\d+)$/.exec(node);
  if (!match) {
    return { canInterface: null, canId: null };
  }
  return {
    canInterface: match[2] ?? null,
    canId: Number(match[3]),
  };
}
