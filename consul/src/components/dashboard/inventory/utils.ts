import type { Row, Table } from '@tanstack/react-table';

import {
  INVENTORY_GROUP_LABELS,
  INVENTORY_GROUP_ORDER,
  type InventoryGroup,
  type InventoryItem,
} from '@/data/robot-inventory';

import type { InventoryGroupSection, InventoryView } from '@/components/dashboard/inventory/types';

export function isHealthyStatus(status: string): boolean {
  return status === 'Enabled' || status === 'Nominal';
}

export function filterInventoryByView(
  data: InventoryItem[],
  view: InventoryView,
): InventoryItem[] {
  switch (view) {
    case 'faults':
      return data.filter((item) => item.status === 'Fault');
    case 'offline':
      return data.filter((item) => item.status === 'Offline');
    case 'unconfigured':
      return data.filter((item) => item.preset === 'unassigned');
    default:
      return data;
  }
}

export function buildGroupedSections(rows: Row<InventoryItem>[]): InventoryGroupSection[] {
  return INVENTORY_GROUP_ORDER.map((group) => ({
    group,
    label: INVENTORY_GROUP_LABELS[group],
    rows: rows.filter((row) => row.original.group === group),
  })).filter((section) => section.rows.length > 0);
}

export function getVisibleRowIds(
  data: InventoryItem[],
  collapsedGroups: Set<InventoryGroup>,
): number[] {
  return data
    .filter((item) => !collapsedGroups.has(item.group))
    .map(({ id }) => id);
}

export function countExpandedGroups(
  sections: InventoryGroupSection[],
  collapsedGroups: Set<InventoryGroup>,
): number {
  return sections.filter((section) => !collapsedGroups.has(section.group)).length;
}

export type InventoryTable = Table<InventoryItem>;
