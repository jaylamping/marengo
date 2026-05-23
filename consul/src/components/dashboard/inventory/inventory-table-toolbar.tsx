import { INVENTORY_VIEW_OPTIONS } from '@/components/dashboard/inventory/constants';
import type { InventoryView } from '@/components/dashboard/inventory/types';
import type { InventoryTable } from '@/components/dashboard/inventory/utils';
import { Badge } from '@/components/ui/badge';
import { Button } from '@/components/ui/button';
import {
  DropdownMenu,
  DropdownMenuCheckboxItem,
  DropdownMenuContent,
  DropdownMenuTrigger,
} from '@/components/ui/dropdown-menu';
import { Label } from '@/components/ui/label';
import {
  Select,
  SelectContent,
  SelectGroup,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from '@/components/ui/select';
import { TabsList, TabsTrigger } from '@/components/ui/tabs';
import { HugeiconsIcon } from '@hugeicons/react';
import {
  Add01Icon,
  ArrowDown01Icon,
  LeftToRightListBulletIcon,
} from '@hugeicons/core-free-icons';

type InventoryTableToolbarProps = {
  activeView: InventoryView;
  onViewChange: (view: InventoryView) => void;
  viewCounts: {
    faults: number;
    offline: number;
    unconfigured: number;
  };
  table: InventoryTable;
  onExpandAll: () => void;
  onCollapseAll: () => void;
};

export function InventoryTableToolbar({
  activeView,
  onViewChange,
  viewCounts,
  table,
  onExpandAll,
  onCollapseAll,
}: InventoryTableToolbarProps) {
  return (
    <div className="flex items-center justify-between px-4 lg:px-6">
      <Label htmlFor="view-selector" className="sr-only">
        View
      </Label>
      <Select
        value={activeView}
        onValueChange={(value) => {
          if (value !== null) {
            onViewChange(value as InventoryView);
          }
        }}
        items={[...INVENTORY_VIEW_OPTIONS]}
      >
        <SelectTrigger
          className="flex w-fit @4xl/main:hidden"
          size="sm"
          id="view-selector"
        >
          <SelectValue placeholder="Select a view" />
        </SelectTrigger>
        <SelectContent>
          <SelectGroup>
            {INVENTORY_VIEW_OPTIONS.map((option) => (
              <SelectItem key={option.value} value={option.value}>
                {option.label}
              </SelectItem>
            ))}
          </SelectGroup>
        </SelectContent>
      </Select>
      <TabsList className="hidden **:data-[slot=badge]:size-5 **:data-[slot=badge]:rounded-full **:data-[slot=badge]:bg-muted-foreground/30 **:data-[slot=badge]:px-1 @4xl/main:flex">
        <TabsTrigger value="all">All Devices</TabsTrigger>
        <TabsTrigger value="faults">
          Faults <Badge variant="secondary">{viewCounts.faults}</Badge>
        </TabsTrigger>
        <TabsTrigger value="offline">
          Offline <Badge variant="secondary">{viewCounts.offline}</Badge>
        </TabsTrigger>
        <TabsTrigger value="unconfigured">
          Unconfigured <Badge variant="secondary">{viewCounts.unconfigured}</Badge>
        </TabsTrigger>
      </TabsList>
      <div className="flex items-center gap-2">
        <DropdownMenu>
          <DropdownMenuTrigger render={<Button variant="outline" size="sm" />}>
            <HugeiconsIcon
              icon={LeftToRightListBulletIcon}
              strokeWidth={2}
              data-icon="inline-start"
            />
            Columns
            <HugeiconsIcon
              icon={ArrowDown01Icon}
              strokeWidth={2}
              data-icon="inline-end"
            />
          </DropdownMenuTrigger>
          <DropdownMenuContent align="end" className="w-32">
            {table
              .getAllColumns()
              .filter(
                (column) =>
                  typeof column.accessorFn !== 'undefined' &&
                  column.getCanHide(),
              )
              .map((column) => (
                <DropdownMenuCheckboxItem
                  key={column.id}
                  className="capitalize"
                  checked={column.getIsVisible()}
                  onCheckedChange={(value) => column.toggleVisibility(!!value)}
                >
                  {column.id}
                </DropdownMenuCheckboxItem>
              ))}
          </DropdownMenuContent>
        </DropdownMenu>
        <Button variant="ghost" size="sm" onClick={onExpandAll}>
          Expand all
        </Button>
        <Button variant="ghost" size="sm" onClick={onCollapseAll}>
          Collapse all
        </Button>
        <Button variant="outline" size="sm">
          <HugeiconsIcon icon={Add01Icon} strokeWidth={2} />
          <span className="hidden lg:inline">Add Device</span>
        </Button>
      </div>
    </div>
  );
}
