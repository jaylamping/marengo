import type { Row } from '@tanstack/react-table';

import type {
  InventoryGroup,
  InventoryItem,
  InventoryKind,
  InventoryStatus,
} from '@/data/robot-inventory';

export type InventoryRow = InventoryItem;

/** Operator identity fields editable from the inventory table/modal. */
export type InventoryIdentityPatch = {
  name: string;
  group: InventoryGroup;
  kind: InventoryKind;
  status: InventoryStatus;
  preset: string;
  limit: string;
};

export type InventoryView = 'all' | 'faults' | 'offline' | 'unconfigured';

export type InventoryGroupSection = {
  group: InventoryGroup;
  label: string;
  rows: Row<InventoryRow>[];
};
