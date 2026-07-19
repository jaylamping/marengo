import * as React from 'react';
import { flexRender } from '@tanstack/react-table';

import { inventoryColumnCount } from '@/components/dashboard/inventory/inventory-columns';
import { inventoryTableShellClassName } from '@/components/dashboard/inventory/constants';
import { InventoryGroupHeaderRow } from '@/components/dashboard/inventory/inventory-group-header-row';
import { InventoryTableRow } from '@/components/dashboard/inventory/inventory-table-row';
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
  onToggleGroup: (group: InventoryGroup) => void;
};

export function InventoryTableView({
  table,
  groupedSections,
  collapsedGroups,
  onToggleGroup,
}: InventoryTableViewProps) {
  return (
    <div
      className={inventoryTableShellClassName}
      data-testid="inventory-table-shell"
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
            groupedSections.map(({ group, label, rows }) => {
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
                        <InventoryTableRow key={row.id} row={row} />
                      ))
                    : null}
                </React.Fragment>
              );
            })
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
    </div>
  );
}
