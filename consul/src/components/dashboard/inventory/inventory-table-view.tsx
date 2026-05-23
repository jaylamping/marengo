import * as React from 'react';
import {
  DndContext,
  closestCenter,
  type DragEndEvent,
  type SensorDescriptor,
  type SensorOptions,
  type UniqueIdentifier,
} from '@dnd-kit/core';
import { restrictToVerticalAxis } from '@dnd-kit/modifiers';
import { SortableContext, verticalListSortingStrategy } from '@dnd-kit/sortable';
import { flexRender } from '@tanstack/react-table';

import {
  inventoryColumnCount,
} from '@/components/dashboard/inventory/inventory-columns';
import { InventoryDraggableRow } from '@/components/dashboard/inventory/inventory-draggable-row';
import { InventoryGroupHeaderRow } from '@/components/dashboard/inventory/inventory-group-header-row';
import type { InventoryGroupSection } from '@/components/dashboard/inventory/types';
import type { InventoryGroup } from '@/data/robot-inventory';
import type { InventoryTable } from '@/components/dashboard/inventory/utils';
import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from '@/components/ui/table';

type InventoryTableViewProps = {
  table: InventoryTable;
  groupedSections: InventoryGroupSection[];
  collapsedGroups: Set<InventoryGroup>;
  dataIds: UniqueIdentifier[];
  sortableId: string;
  sensors: SensorDescriptor<SensorOptions>[];
  onDragEnd: (event: DragEndEvent) => void;
  onToggleGroup: (group: InventoryGroup) => void;
};

export function InventoryTableView({
  table,
  groupedSections,
  collapsedGroups,
  dataIds,
  sortableId,
  sensors,
  onDragEnd,
  onToggleGroup,
}: InventoryTableViewProps) {
  return (
    <div className="overflow-hidden rounded-lg border">
      <DndContext
        collisionDetection={closestCenter}
        modifiers={[restrictToVerticalAxis]}
        onDragEnd={onDragEnd}
        sensors={sensors}
        id={sortableId}
      >
        <Table>
          <TableHeader className="sticky top-0 z-10 bg-muted">
            {table.getHeaderGroups().map((headerGroup) => (
              <TableRow key={headerGroup.id}>
                {headerGroup.headers.map((header) => (
                  <TableHead key={header.id} colSpan={header.colSpan}>
                    {header.isPlaceholder
                      ? null
                      : flexRender(
                          header.column.columnDef.header,
                          header.getContext(),
                        )}
                  </TableHead>
                ))}
              </TableRow>
            ))}
          </TableHeader>
          <TableBody className="**:data-[slot=table-cell]:first:w-8">
            {groupedSections.length ? (
              <SortableContext
                items={dataIds}
                strategy={verticalListSortingStrategy}
              >
                {groupedSections.map(({ group, label, rows }) => {
                  const isCollapsed = collapsedGroups.has(group);

                  return (
                    <React.Fragment key={group}>
                      <InventoryGroupHeaderRow
                        group={group}
                        label={label}
                        rowCount={rows.length}
                        columnCount={inventoryColumnCount}
                        isCollapsed={isCollapsed}
                        onToggle={() => onToggleGroup(group)}
                      />
                      {!isCollapsed
                        ? rows.map((row) => (
                            <InventoryDraggableRow key={row.id} row={row} />
                          ))
                        : null}
                    </React.Fragment>
                  );
                })}
              </SortableContext>
            ) : (
              <TableRow>
                <TableCell
                  colSpan={inventoryColumnCount}
                  className="h-24 text-center"
                >
                  No devices match this view.
                </TableCell>
              </TableRow>
            )}
          </TableBody>
        </Table>
      </DndContext>
    </div>
  );
}
