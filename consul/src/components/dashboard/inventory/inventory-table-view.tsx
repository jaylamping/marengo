import * as React from 'react';
import { flexRender } from '@tanstack/react-table';

import {
  inventoryColumnCount,
  type InventoryColumnMeta,
} from '@/components/dashboard/inventory/inventory-columns';
import { inventoryTableShellClassName } from '@/components/dashboard/inventory/constants';
import { InventoryGroupHeaderRow } from '@/components/dashboard/inventory/inventory-group-header-row';
import { InventoryTableRow } from '@/components/dashboard/inventory/inventory-table-row';
import type { InventoryGroupSection } from '@/components/dashboard/inventory/types';
import type { InventoryGroup } from '@/data/robot-inventory';
import type { InventoryTable } from '@/components/dashboard/inventory/utils';
import { cn } from '@/lib/utils';
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

function columnMetaClassName(meta: unknown): string | undefined {
  return (meta as InventoryColumnMeta | undefined)?.className;
}

function columnSize(column: {
  getSize?: () => number;
  columnDef: { size?: number };
}): number | undefined {
  if (typeof column.getSize === 'function') {
    return column.getSize();
  }
  return column.columnDef.size;
}

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
      <Table className="table-fixed">
        <TableHeader className="sticky top-0 z-10 border-b border-line">
          {table.getHeaderGroups().map((headerGroup) => (
            <TableRow key={headerGroup.id} className="hover:bg-transparent">
              {headerGroup.headers.map((header) => (
                <TableHead
                  key={header.id}
                  colSpan={header.colSpan}
                  className={cn(
                    'bg-surface-2',
                    columnMetaClassName(header.column.columnDef.meta),
                  )}
                  style={{ width: columnSize(header.column) }}
                >
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
        <TableBody>
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
                No devices in this view.
              </TableCell>
            </TableRow>
          )}
        </TableBody>
      </Table>
    </div>
  );
}
