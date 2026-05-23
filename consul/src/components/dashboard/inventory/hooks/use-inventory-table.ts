import * as React from 'react';
import {
  KeyboardSensor,
  MouseSensor,
  TouchSensor,
  useSensor,
  useSensors,
  type DragEndEvent,
  type UniqueIdentifier,
} from '@dnd-kit/core';
import { arrayMove } from '@dnd-kit/sortable';
import {
  getCoreRowModel,
  getFacetedRowModel,
  getFacetedUniqueValues,
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
  getVisibleRowIds,
} from '@/components/dashboard/inventory/utils';

export function useInventoryTable(initialData: InventoryItem[]) {
  const [data, setData] = React.useState(() => initialData);
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
  const sortableId = React.useId();

  const sensors = useSensors(
    useSensor(MouseSensor, {}),
    useSensor(TouchSensor, {}),
    useSensor(KeyboardSensor, {}),
  );

  const filteredData = React.useMemo(
    () => filterInventoryByView(data, activeView),
    [activeView, data],
  );

  const dataIds = React.useMemo<UniqueIdentifier[]>(
    () => getVisibleRowIds(filteredData, collapsedGroups),
    [collapsedGroups, filteredData],
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
    getFacetedRowModel: getFacetedRowModel(),
    getFacetedUniqueValues: getFacetedUniqueValues(),
  });

  const groupedSections = React.useMemo(
    () => buildGroupedSections(table.getRowModel().rows),
    [table],
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

  const handleDragEnd = React.useCallback(
    (event: DragEndEvent) => {
      const { active, over } = event;
      if (!active || !over || active.id === over.id) {
        return;
      }

      setData((current) => {
        const oldIndex = dataIds.indexOf(active.id);
        const newIndex = dataIds.indexOf(over.id);
        return arrayMove(current, oldIndex, newIndex);
      });
    },
    [dataIds],
  );

  return {
    activeView,
    setActiveView,
    collapsedGroups,
    data,
    dataIds,
    expandedGroupCount,
    groupedSections,
    handleDragEnd,
    sensors,
    sortableId,
    table,
    toggleGroup,
    expandAllGroups,
    collapseAllGroups,
    viewCounts,
  };
}
