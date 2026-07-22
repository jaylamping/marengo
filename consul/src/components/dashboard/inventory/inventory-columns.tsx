import type { ColumnDef } from '@tanstack/react-table';

import { EditableFieldCell } from '@/components/dashboard/inventory/cells/editable-field-cell';
import { InventoryNameCell } from '@/components/dashboard/inventory/cells/inventory-name-cell';
import { InventoryStatusCell } from '@/components/dashboard/inventory/cells/inventory-status-cell';
import { PresetCell } from '@/components/dashboard/inventory/cells/preset-cell';
import { RowActionsMenu } from '@/components/dashboard/inventory/cells/row-actions-menu';
import type { InventoryRow } from '@/components/dashboard/inventory/types';
import { NeedsRestartBadge } from '@/components/dashboard/needs-restart/needs-restart-badge';
import { Badge } from '@/components/ui/badge';
import { Checkbox } from '@/components/ui/checkbox';

export type InventoryColumnMeta = {
  className?: string;
};

export const inventoryColumns: ColumnDef<InventoryRow>[] = [
  {
    id: 'select',
    size: 40,
    header: ({ table }) => (
      <div className="flex items-center justify-center">
        <Checkbox
          checked={table.getIsAllPageRowsSelected()}
          indeterminate={
            table.getIsSomePageRowsSelected() &&
            !table.getIsAllPageRowsSelected()
          }
          onCheckedChange={(value) => table.toggleAllPageRowsSelected(!!value)}
          aria-label="Select all"
        />
      </div>
    ),
    cell: ({ row }) => (
      <div className="flex items-center justify-center">
        <Checkbox
          checked={row.getIsSelected()}
          onCheckedChange={(value) => row.toggleSelected(!!value)}
          aria-label="Select row"
        />
      </div>
    ),
    enableSorting: false,
    enableHiding: false,
  },
  {
    accessorKey: 'name',
    size: 260,
    header: 'Name',
    cell: ({ row }) => <InventoryNameCell item={row.original} />,
    enableHiding: false,
  },
  {
    accessorKey: 'kind',
    size: 100,
    header: 'Kind',
    cell: ({ row }) => (
      <Badge
        variant="outline"
        className="px-1.5 font-mono text-[10px] capitalize tracking-[0.04em] text-muted-foreground"
      >
        {row.original.kind}
      </Badge>
    ),
  },
  {
    accessorKey: 'status',
    size: 120,
    header: 'Status',
    cell: ({ row }) => <InventoryStatusCell status={row.original.status} />,
  },
  {
    accessorKey: 'value',
    size: 110,
    meta: { className: 'text-right' } satisfies InventoryColumnMeta,
    header: () => <span className="block w-full text-right">Reading</span>,
    cell: ({ row }) => (
      <EditableFieldCell
        itemId={row.original.id}
        itemName={row.original.name}
        field="value"
        label="Reading"
        defaultValue={row.original.value}
      />
    ),
  },
  {
    accessorKey: 'limit',
    size: 120,
    meta: { className: 'text-right' } satisfies InventoryColumnMeta,
    header: () => <span className="block w-full text-right">Range</span>,
    cell: ({ row }) => (
      <div className="flex items-center justify-end gap-1.5">
        <NeedsRestartBadge
          variant="pending"
          jointName={row.original.name}
        />
        <EditableFieldCell
          itemId={row.original.id}
          itemName={row.original.name}
          field="limit"
          label="Range"
          defaultValue={row.original.limit}
          inputClassName="h-8 w-24"
        />
      </div>
    ),
  },
  {
    accessorKey: 'preset',
    size: 150,
    header: 'Preset',
    cell: ({ row }) => (
      <PresetCell
        itemId={row.original.id}
        preset={row.original.preset}
        jointName={row.original.name}
      />
    ),
  },
  {
    id: 'actions',
    size: 48,
    meta: { className: 'text-right' } satisfies InventoryColumnMeta,
    cell: () => <RowActionsMenu />,
  },
];

export const inventoryColumnCount = inventoryColumns.length;
