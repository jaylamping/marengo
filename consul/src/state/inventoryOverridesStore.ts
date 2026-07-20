import { create as createZustand } from 'zustand';

import type { InventoryIdentityPatch, InventoryRow } from '@/components/dashboard/inventory/types';
import {
  INVENTORY_GROUP_ORDER,
  type InventoryGroup,
  type InventoryItem,
  type InventoryKind,
  type InventoryStatus,
} from '@/data/robot-inventory';

export const INVENTORY_OVERRIDES_STORAGE_KEY = 'marengo.inventory.overrides.v1';

export type InventoryIdentityOverrides = Record<number, Partial<InventoryIdentityPatch>>;

export type InventoryOverridesPersisted = {
  version: 1;
  overrides: InventoryIdentityOverrides;
};

const INVENTORY_GROUPS = new Set<string>(INVENTORY_GROUP_ORDER);
const INVENTORY_KINDS = new Set<string>(['actuator', 'sensor', 'device']);
const INVENTORY_STATUSES = new Set<string>([
  'Enabled',
  'Tuning',
  'Fault',
  'Offline',
  'Nominal',
]);

const emptyPersisted = (): InventoryOverridesPersisted => ({ version: 1, overrides: {} });

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === 'object' && value !== null && !Array.isArray(value);
}

function parsePatch(value: unknown): Partial<InventoryIdentityPatch> | null {
  if (!isRecord(value)) {
    return null;
  }

  const patch: Partial<InventoryIdentityPatch> = {};
  if (typeof value.name === 'string') patch.name = value.name;
  if (typeof value.group === 'string' && INVENTORY_GROUPS.has(value.group)) {
    patch.group = value.group as InventoryGroup;
  }
  if (typeof value.kind === 'string' && INVENTORY_KINDS.has(value.kind)) {
    patch.kind = value.kind as InventoryKind;
  }
  if (typeof value.status === 'string' && INVENTORY_STATUSES.has(value.status)) {
    patch.status = value.status as InventoryStatus;
  }
  if (typeof value.preset === 'string') patch.preset = value.preset;
  if (typeof value.limit === 'string') patch.limit = value.limit;
  return Object.keys(patch).length > 0 ? patch : null;
}

/** Parses only versioned catalog identity patches from browser storage. */
export function parseInventoryOverridesPersisted(raw: string | null): InventoryOverridesPersisted {
  if (!raw) {
    return emptyPersisted();
  }

  try {
    const parsed: unknown = JSON.parse(raw);
    if (!isRecord(parsed) || parsed.version !== 1 || !isRecord(parsed.overrides)) {
      return emptyPersisted();
    }

    const overrides: InventoryIdentityOverrides = {};
    for (const [itemId, patch] of Object.entries(parsed.overrides)) {
      const id = Number(itemId);
      const parsedPatch = parsePatch(patch);
      if (Number.isInteger(id) && id >= 0 && parsedPatch !== null) {
        overrides[id] = parsedPatch;
      }
    }
    return { version: 1, overrides };
  } catch {
    return emptyPersisted();
  }
}

function loadPersisted(): InventoryOverridesPersisted {
  if (typeof window === 'undefined') {
    return emptyPersisted();
  }
  return parseInventoryOverridesPersisted(
    window.localStorage.getItem(INVENTORY_OVERRIDES_STORAGE_KEY),
  );
}

function persist(overrides: InventoryIdentityOverrides) {
  if (typeof window === 'undefined') {
    return;
  }
  try {
    window.localStorage.setItem(
      INVENTORY_OVERRIDES_STORAGE_KEY,
      JSON.stringify({ version: 1, overrides } satisfies InventoryOverridesPersisted),
    );
  } catch {
    // Quota / private mode — keep in-memory overrides; next Apply can retry.
  }
}

export function applyOverrides(
  item: InventoryItem,
  overrides: InventoryIdentityOverrides,
): InventoryRow {
  const patch = overrides[item.id];
  return patch === undefined ? item : { ...item, ...patch };
}

type InventoryOverridesStore = {
  overrides: InventoryIdentityOverrides;
  applyPatch: (itemId: number, patch: Partial<InventoryIdentityPatch>) => void;
};

const initial = loadPersisted();

export const useInventoryOverridesStore = createZustand<InventoryOverridesStore>((set, get) => ({
  overrides: initial.overrides,
  applyPatch: (itemId, patch) => {
    const overrides = {
      ...get().overrides,
      [itemId]: { ...get().overrides[itemId], ...patch },
    };
    persist(overrides);
    set({ overrides });
  },
}));
