import type { Row } from '@tanstack/react-table';

import type { InventoryGroup, InventoryItem } from '@/data/robot-inventory';

export type InventoryRow = InventoryItem;

export type InventoryView = 'all' | 'faults' | 'offline' | 'unconfigured';

export type InventoryGroupSection = {
  group: InventoryGroup;
  label: string;
  rows: Row<InventoryRow>[];
};
