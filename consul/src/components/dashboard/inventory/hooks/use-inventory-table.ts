import * as React from 'react';
import {
  getCoreRowModel,
  getFilteredRowModel,
  getSortedRowModel,
  useReactTable,
  type ColumnFiltersState,
  type SortingState,
  type VisibilityState,
} from '@tanstack/react-table';

import {
  INVENTORY_GROUP_ORDER,
  countByStatus,
  countUnconfigured,
  type InventoryGroup,
  type InventoryItem,
} from '@/data/robot-inventory';

import { inventoryColumns } from '@/components/dashboard/inventory/inventory-columns';
import type { InventoryView } from '@/components/dashboard/inventory/types';
import {
  buildGroupedSections,
  countExpandedGroups,
  filterInventoryByView,
} from '@/components/dashboard/inventory/utils';

export function useInventoryTable(data: InventoryItem[]) {
  // Parent owns data stability (enriched query / static catalog — not live Chappe).
  const [activeView, setActiveView] = React.useState<InventoryView>('all');
  const [collapsedGroups, setCollapsedGroups] = React.useState<
    Set<InventoryGroup>
  >(() => new Set());
  const [rowSelection, setRowSelection] = React.useState({});
  const [columnVisibility, setColumnVisibility] =
    React.useState<VisibilityState>({});
  const [columnFilters, setColumnFilters] = React.useState<ColumnFiltersState>(
    [],
  );
  const [sorting, setSorting] = React.useState<SortingState>([]);

  const filteredData = React.useMemo(
    () => filterInventoryByView(data, activeView),
    [activeView, data],
  );

  const table = useReactTable({
    data: filteredData,
    columns: inventoryColumns,
    state: {
      sorting,
      columnVisibility,
      rowSelection,
      columnFilters,
    },
    getRowId: (row) => row.id.toString(),
    enableRowSelection: true,
    onRowSelectionChange: setRowSelection,
    onSortingChange: setSorting,
    onColumnFiltersChange: setColumnFilters,
    onColumnVisibilityChange: setColumnVisibility,
    getCoreRowModel: getCoreRowModel(),
    getFilteredRowModel: getFilteredRowModel(),
    getSortedRowModel: getSortedRowModel(),
  });

  const rowModel = table.getRowModel();
  const groupedSections = React.useMemo(
    () => buildGroupedSections(rowModel.rows),
    [rowModel.rows],
  );

  const viewCounts = React.useMemo(
    () => ({
      faults: countByStatus('Fault'),
      offline: countByStatus('Offline'),
      unconfigured: countUnconfigured(),
    }),
    [],
  );

  const expandedGroupCount = countExpandedGroups(
    groupedSections,
    collapsedGroups,
  );

  const toggleGroup = React.useCallback((group: InventoryGroup) => {
    setCollapsedGroups((previous) => {
      const next = new Set(previous);
      if (next.has(group)) {
        next.delete(group);
      } else {
        next.add(group);
      }
      return next;
    });
  }, []);

  const expandAllGroups = React.useCallback(() => {
    setCollapsedGroups(new Set());
  }, []);

  const collapseAllGroups = React.useCallback(() => {
    setCollapsedGroups(new Set(INVENTORY_GROUP_ORDER));
  }, []);

  return {
    activeView,
    setActiveView,
    collapsedGroups,
    data,
    expandedGroupCount,
    groupedSections,
    table,
    toggleGroup,
    expandAllGroups,
    collapseAllGroups,
    viewCounts,
  };
}
