import type { ColumnDef } from '@tanstack/react-table';

import { DragHandle } from '@/components/dashboard/inventory/cells/drag-handle';
import { EditableFieldCell } from '@/components/dashboard/inventory/cells/editable-field-cell';
import { InventoryNameCell } from '@/components/dashboard/inventory/cells/inventory-name-cell';
import { InventoryStatusCell } from '@/components/dashboard/inventory/cells/inventory-status-cell';
import { PresetCell } from '@/components/dashboard/inventory/cells/preset-cell';
import { RowActionsMenu } from '@/components/dashboard/inventory/cells/row-actions-menu';
import type { InventoryRow } from '@/components/dashboard/inventory/types';
import { Badge } from '@/components/ui/badge';
import { Checkbox } from '@/components/ui/checkbox';

export const inventoryColumns: ColumnDef<InventoryRow>[] = [
  {
    id: 'drag',
    header: () => null,
    cell: ({ row }) => <DragHandle id={row.original.id} />,
  },
  {
    id: 'select',
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
    header: 'Name',
    cell: ({ row }) => <InventoryNameCell item={row.original} />,
    enableHiding: false,
  },
  {
    accessorKey: 'kind',
    header: 'Kind',
    cell: ({ row }) => (
      <Badge variant="outline" className="px-1.5 capitalize text-muted-foreground">
        {row.original.kind}
      </Badge>
    ),
  },
  {
    accessorKey: 'status',
    header: 'Status',
    cell: ({ row }) => <InventoryStatusCell status={row.original.status} />,
  },
  {
    accessorKey: 'value',
    header: () => <div className="w-full text-right">Reading</div>,
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
    header: () => <div className="w-full text-right">Range</div>,
    cell: ({ row }) => (
      <EditableFieldCell
        itemId={row.original.id}
        itemName={row.original.name}
        field="limit"
        label="Range"
        defaultValue={row.original.limit}
        inputClassName="h-8 w-24"
      />
    ),
  },
  {
    accessorKey: 'preset',
    header: 'Preset',
    cell: ({ row }) => (
      <PresetCell itemId={row.original.id} preset={row.original.preset} />
    ),
  },
  {
    id: 'actions',
    cell: () => <RowActionsMenu />,
  },
];

export const inventoryColumnCount = inventoryColumns.length;
