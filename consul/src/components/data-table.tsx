import * as React from "react";
import {
  closestCenter,
  DndContext,
  KeyboardSensor,
  MouseSensor,
  TouchSensor,
  useSensor,
  useSensors,
  type DragEndEvent,
  type UniqueIdentifier,
} from "@dnd-kit/core";
import { restrictToVerticalAxis } from "@dnd-kit/modifiers";
import {
  arrayMove,
  SortableContext,
  useSortable,
  verticalListSortingStrategy,
} from "@dnd-kit/sortable";
import { CSS } from "@dnd-kit/utilities";
import {
  flexRender,
  getCoreRowModel,
  getFacetedRowModel,
  getFacetedUniqueValues,
  getFilteredRowModel,
  getSortedRowModel,
  useReactTable,
  type ColumnDef,
  type ColumnFiltersState,
  type Row,
  type SortingState,
  type VisibilityState,
} from "@tanstack/react-table";
import { Area, AreaChart, CartesianGrid, XAxis } from "recharts";
import { toast } from "sonner";
import { z } from "zod";

import {
  INVENTORY_GROUP_LABELS,
  INVENTORY_GROUP_ORDER,
  countByStatus,
  countUnconfigured,
  type InventoryGroup,
  type InventoryItem,
} from "@/data/robot-inventory";
import { cn } from "@/lib/utils";

import { useIsMobile } from "@/hooks/use-mobile";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import {
  ChartContainer,
  ChartTooltip,
  ChartTooltipContent,
  type ChartConfig,
} from "@/components/ui/chart";
import { Checkbox } from "@/components/ui/checkbox";
import {
  Drawer,
  DrawerClose,
  DrawerContent,
  DrawerDescription,
  DrawerFooter,
  DrawerHeader,
  DrawerTitle,
  DrawerTrigger,
} from "@/components/ui/drawer";
import {
  DropdownMenu,
  DropdownMenuCheckboxItem,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuSeparator,
  DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import {
  Select,
  SelectContent,
  SelectGroup,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import { Separator } from "@/components/ui/separator";
import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from "@/components/ui/table";
import { Tabs, TabsList, TabsTrigger } from "@/components/ui/tabs";
import { HugeiconsIcon } from "@hugeicons/react";
import {
  DragDropVerticalIcon,
  CheckmarkCircle01Icon,
  Loading03Icon,
  MoreVerticalCircle01Icon,
  LeftToRightListBulletIcon,
  ArrowDown01Icon,
  Add01Icon,
  ChartUpIcon,
} from "@hugeicons/core-free-icons";

export const schema = z.object({
  id: z.number(),
  name: z.string(),
  group: z.string(),
  kind: z.string(),
  status: z.string(),
  value: z.string(),
  limit: z.string(),
  preset: z.string(),
  node: z.string(),
});

export type InventoryRow = z.infer<typeof schema>;

function isHealthyStatus(status: string) {
  return status === "Enabled" || status === "Nominal";
}

// Create a separate component for the drag handle
function DragHandle({ id }: { id: number }) {
  const { attributes, listeners } = useSortable({
    id,
  });
  return (
    <Button
      {...attributes}
      {...listeners}
      variant="ghost"
      size="icon"
      className="size-7 text-muted-foreground hover:bg-transparent"
    >
      <HugeiconsIcon
        icon={DragDropVerticalIcon}
        strokeWidth={2}
        className="size-3 text-muted-foreground"
      />
      <span className="sr-only">Drag to reorder</span>
    </Button>
  );
}
const columns: ColumnDef<InventoryRow>[] = [
  {
    id: "drag",
    header: () => null,
    cell: ({ row }) => <DragHandle id={row.original.id} />,
  },
  {
    id: "select",
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
    accessorKey: "name",
    header: "Name",
    cell: ({ row }) => (
      <div className="min-w-[10rem]">
        <TableCellViewer item={row.original} />
        <div className="mt-0.5 font-mono text-[10px] text-muted-foreground">
          {row.original.node}
        </div>
      </div>
    ),
    enableHiding: false,
  },
  {
    accessorKey: "kind",
    header: "Kind",
    cell: ({ row }) => (
      <Badge
        variant="outline"
        className="px-1.5 capitalize text-muted-foreground"
      >
        {row.original.kind}
      </Badge>
    ),
  },
  {
    accessorKey: "status",
    header: "Status",
    cell: ({ row }) => (
      <Badge variant="outline" className="px-1.5 text-muted-foreground">
        {isHealthyStatus(row.original.status) ? (
          <HugeiconsIcon
            icon={CheckmarkCircle01Icon}
            strokeWidth={2}
            className="fill-green-500 dark:fill-green-400"
          />
        ) : row.original.status === "Tuning" ? (
          <HugeiconsIcon icon={Loading03Icon} strokeWidth={2} />
        ) : row.original.status === "Fault" ? (
          <HugeiconsIcon
            icon={Loading03Icon}
            strokeWidth={2}
            className="text-destructive"
          />
        ) : null}
        {row.original.status}
      </Badge>
    ),
  },
  {
    accessorKey: "value",
    header: () => <div className="w-full text-right">Reading</div>,
    cell: ({ row }) => (
      <form
        onSubmit={(e) => {
          e.preventDefault();
          toast.promise(new Promise((resolve) => setTimeout(resolve, 1000)), {
            loading: `Saving ${row.original.name}`,
            success: "Done",
            error: "Error",
          });
        }}
      >
        <Label htmlFor={`${row.original.id}-value`} className="sr-only">
          Reading
        </Label>
        <Input
          className="h-8 w-20 border-transparent bg-transparent text-right font-mono text-xs shadow-none hover:bg-input/30 focus-visible:border focus-visible:bg-background dark:bg-transparent dark:hover:bg-input/30 dark:focus-visible:bg-input/30"
          defaultValue={row.original.value}
          id={`${row.original.id}-value`}
        />
      </form>
    ),
  },
  {
    accessorKey: "limit",
    header: () => <div className="w-full text-right">Range</div>,
    cell: ({ row }) => (
      <form
        onSubmit={(e) => {
          e.preventDefault();
          toast.promise(new Promise((resolve) => setTimeout(resolve, 1000)), {
            loading: `Saving ${row.original.name}`,
            success: "Done",
            error: "Error",
          });
        }}
      >
        <Label htmlFor={`${row.original.id}-limit`} className="sr-only">
          Range
        </Label>
        <Input
          className="h-8 w-24 border-transparent bg-transparent text-right font-mono text-xs shadow-none hover:bg-input/30 focus-visible:border focus-visible:bg-background dark:bg-transparent dark:hover:bg-input/30 dark:focus-visible:bg-input/30"
          defaultValue={row.original.limit}
          id={`${row.original.id}-limit`}
        />
      </form>
    ),
  },
  {
    accessorKey: "preset",
    header: "Preset",
    cell: ({ row }) => {
      const isAssigned = row.original.preset !== "unassigned";
      if (isAssigned) {
        return <span className="font-mono text-xs">{row.original.preset}</span>;
      }
      return (
        <>
          <Label htmlFor={`${row.original.id}-preset`} className="sr-only">
            Preset
          </Label>
          <Select
            items={[
              { label: "golden_pose", value: "golden_pose" },
              { label: "bench_default", value: "bench_default" },
              { label: "tuning_sweep", value: "tuning_sweep" },
              { label: "last_session", value: "last_session" },
            ]}
          >
            <SelectTrigger
              className="w-38 **:data-[slot=select-value]:block **:data-[slot=select-value]:truncate"
              size="sm"
              id={`${row.original.id}-preset`}
            >
              <SelectValue placeholder="Assign preset" />
            </SelectTrigger>
            <SelectContent align="end">
              <SelectGroup>
                <SelectItem value="golden_pose">golden_pose</SelectItem>
                <SelectItem value="bench_default">bench_default</SelectItem>
                <SelectItem value="tuning_sweep">tuning_sweep</SelectItem>
                <SelectItem value="last_session">last_session</SelectItem>
              </SelectGroup>
            </SelectContent>
          </Select>
        </>
      );
    },
  },
  {
    id: "actions",
    cell: () => (
      <DropdownMenu>
        <DropdownMenuTrigger
          render={
            <Button
              variant="ghost"
              className="flex size-8 text-muted-foreground data-open:bg-muted"
              size="icon"
            />
          }
        >
          <HugeiconsIcon icon={MoreVerticalCircle01Icon} strokeWidth={2} />
          <span className="sr-only">Open menu</span>
        </DropdownMenuTrigger>
        <DropdownMenuContent align="end" className="w-32">
          <DropdownMenuItem>Zero / home</DropdownMenuItem>
          <DropdownMenuItem>Apply preset</DropdownMenuItem>
          <DropdownMenuItem>Disable</DropdownMenuItem>
          <DropdownMenuSeparator />
          <DropdownMenuItem variant="destructive">Clear fault</DropdownMenuItem>
        </DropdownMenuContent>
      </DropdownMenu>
    ),
  },
];
function DraggableRow({ row }: { row: Row<InventoryRow> }) {
  const { transform, transition, setNodeRef, isDragging } = useSortable({
    id: row.original.id,
  });
  return (
    <TableRow
      data-state={row.getIsSelected() && "selected"}
      data-dragging={isDragging}
      ref={setNodeRef}
      className="relative z-0 data-[dragging=true]:z-10 data-[dragging=true]:opacity-80"
      style={{
        transform: CSS.Transform.toString(transform),
        transition: transition,
      }}
    >
      {row.getVisibleCells().map((cell) => (
        <TableCell key={cell.id}>
          {flexRender(cell.column.columnDef.cell, cell.getContext())}
        </TableCell>
      ))}
    </TableRow>
  );
}
export function DataTable({ data: initialData }: { data: InventoryItem[] }) {
  const [data, setData] = React.useState(() => initialData);
  const [activeView, setActiveView] = React.useState("all");
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

  const filteredData = React.useMemo(() => {
    switch (activeView) {
      case "faults":
        return data.filter((item) => item.status === "Fault");
      case "offline":
        return data.filter((item) => item.status === "Offline");
      case "unconfigured":
        return data.filter((item) => item.preset === "unassigned");
      default:
        return data;
    }
  }, [activeView, data]);

  const dataIds = React.useMemo<UniqueIdentifier[]>(() => {
    return filteredData
      .filter((item) => !collapsedGroups.has(item.group))
      .map(({ id }) => id);
  }, [collapsedGroups, filteredData]);

  const toggleGroup = React.useCallback((group: InventoryGroup) => {
    setCollapsedGroups((prev) => {
      const next = new Set(prev);
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

  const table = useReactTable({
    data: filteredData,
    columns,
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

  const groupedSections = React.useMemo(() => {
    const rows = table.getRowModel().rows;
    return INVENTORY_GROUP_ORDER.map((group) => ({
      group,
      label: INVENTORY_GROUP_LABELS[group],
      rows: rows.filter((row) => row.original.group === group),
    })).filter((section) => section.rows.length > 0);
  }, [table]);

  const faultCount = countByStatus("Fault");
  const offlineCount = countByStatus("Offline");
  const unconfiguredCount = countUnconfigured();
  const expandedGroupCount = groupedSections.filter(
    (section) => !collapsedGroups.has(section.group),
  ).length;
  function handleDragEnd(event: DragEndEvent) {
    const { active, over } = event;
    if (active && over && active.id !== over.id) {
      setData((data) => {
        const oldIndex = dataIds.indexOf(active.id);
        const newIndex = dataIds.indexOf(over.id);
        return arrayMove(data, oldIndex, newIndex);
      });
    }
  }
  return (
    <Tabs
      value={activeView}
      onValueChange={setActiveView}
      className="w-full flex-col justify-start gap-6"
    >
      <div className="flex items-center justify-between px-4 lg:px-6">
        <Label htmlFor="view-selector" className="sr-only">
          View
        </Label>
        <Select
          value={activeView}
          onValueChange={(value) => {
            if (value !== null) {
              setActiveView(value);
            }
          }}
          items={[
            { label: "All Devices", value: "all" },
            { label: "Faults", value: "faults" },
            { label: "Offline", value: "offline" },
            { label: "Unconfigured", value: "unconfigured" },
          ]}
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
              <SelectItem value="all">All Devices</SelectItem>
              <SelectItem value="faults">Faults</SelectItem>
              <SelectItem value="offline">Offline</SelectItem>
              <SelectItem value="unconfigured">Unconfigured</SelectItem>
            </SelectGroup>
          </SelectContent>
        </Select>
        <TabsList className="hidden **:data-[slot=badge]:size-5 **:data-[slot=badge]:rounded-full **:data-[slot=badge]:bg-muted-foreground/30 **:data-[slot=badge]:px-1 @4xl/main:flex">
          <TabsTrigger value="all">All Devices</TabsTrigger>
          <TabsTrigger value="faults">
            Faults <Badge variant="secondary">{faultCount}</Badge>
          </TabsTrigger>
          <TabsTrigger value="offline">
            Offline <Badge variant="secondary">{offlineCount}</Badge>
          </TabsTrigger>
          <TabsTrigger value="unconfigured">
            Unconfigured <Badge variant="secondary">{unconfiguredCount}</Badge>
          </TabsTrigger>
        </TabsList>
        <div className="flex items-center gap-2">
          <DropdownMenu>
            <DropdownMenuTrigger
              render={<Button variant="outline" size="sm" />}
            >
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
                    typeof column.accessorFn !== "undefined" &&
                    column.getCanHide(),
                )
                .map((column) => {
                  return (
                    <DropdownMenuCheckboxItem
                      key={column.id}
                      className="capitalize"
                      checked={column.getIsVisible()}
                      onCheckedChange={(value) =>
                        column.toggleVisibility(!!value)
                      }
                    >
                      {column.id}
                    </DropdownMenuCheckboxItem>
                  );
                })}
            </DropdownMenuContent>
          </DropdownMenu>
          <Button variant="ghost" size="sm" onClick={expandAllGroups}>
            Expand all
          </Button>
          <Button variant="ghost" size="sm" onClick={collapseAllGroups}>
            Collapse all
          </Button>
          <Button variant="outline" size="sm">
            <HugeiconsIcon icon={Add01Icon} strokeWidth={2} />
            <span className="hidden lg:inline">Add Device</span>
          </Button>
        </div>
      </div>
      <div className="relative flex flex-col gap-4 overflow-auto px-4 lg:px-6">
        <div className="overflow-hidden rounded-lg border">
          <DndContext
            collisionDetection={closestCenter}
            modifiers={[restrictToVerticalAxis]}
            onDragEnd={handleDragEnd}
            sensors={sensors}
            id={sortableId}
          >
            <Table>
              <TableHeader className="sticky top-0 z-10 bg-muted">
                {table.getHeaderGroups().map((headerGroup) => (
                  <TableRow key={headerGroup.id}>
                    {headerGroup.headers.map((header) => {
                      return (
                        <TableHead key={header.id} colSpan={header.colSpan}>
                          {header.isPlaceholder
                            ? null
                            : flexRender(
                                header.column.columnDef.header,
                                header.getContext(),
                              )}
                        </TableHead>
                      );
                    })}
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
                          <TableRow className="bg-muted/40 hover:bg-muted/60">
                            <TableCell colSpan={columns.length} className="p-0">
                              <button
                                type="button"
                                aria-expanded={!isCollapsed}
                                aria-controls={`inventory-group-${group}`}
                                onClick={() => toggleGroup(group)}
                                className="flex w-full items-center gap-2 px-4 py-2 text-left text-xs font-semibold tracking-wide text-muted-foreground uppercase"
                              >
                                <HugeiconsIcon
                                  icon={ArrowDown01Icon}
                                  strokeWidth={2}
                                  className={cn(
                                    "size-3.5 shrink-0 transition-transform",
                                    isCollapsed && "-rotate-90",
                                  )}
                                />
                                <span>{label}</span>
                                <Badge
                                  variant="secondary"
                                  className="ml-1 font-mono text-[10px]"
                                >
                                  {rows.length}
                                </Badge>
                              </button>
                            </TableCell>
                          </TableRow>
                          {!isCollapsed &&
                            rows.map((row) => (
                              <DraggableRow key={row.id} row={row} />
                            ))}
                        </React.Fragment>
                      );
                    })}
                  </SortableContext>
                ) : (
                  <TableRow>
                    <TableCell
                      colSpan={columns.length}
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
        <div className="flex items-center justify-between text-sm text-muted-foreground">
          <div>
            {table.getFilteredSelectedRowModel().rows.length} of{" "}
            {table.getFilteredRowModel().rows.length} selected ·{" "}
            {groupedSections.length} groups · {expandedGroupCount} expanded
          </div>
          <div>{data.length} total devices (dummy inventory)</div>
        </div>
      </div>
    </Tabs>
  );
}
const chartData = [
  { sample: "0s", commanded: 0.42, measured: 0.38 },
  { sample: "10s", commanded: 0.61, measured: 0.58 },
  { sample: "20s", commanded: 0.72, measured: 0.69 },
  { sample: "30s", commanded: 0.58, measured: 0.55 },
  { sample: "40s", commanded: 0.74, measured: 0.71 },
  { sample: "50s", commanded: 0.85, measured: 0.79 },
  { sample: "60s", commanded: 0.78, measured: 0.73 },
];
const chartConfig = {
  commanded: {
    label: "Commanded (Nm)",
    color: "var(--primary)",
  },
  measured: {
    label: "Measured (Nm)",
    color: "var(--chart-2)",
  },
} satisfies ChartConfig;
function TableCellViewer({ item }: { item: InventoryRow }) {
  const isMobile = useIsMobile();
  return (
    <Drawer direction={isMobile ? "bottom" : "right"}>
      <DrawerTrigger
        render={
          <Button
            variant="link"
            className="w-fit px-0 text-left font-mono text-sm text-foreground"
          />
        }
      >
        {item.name}
      </DrawerTrigger>
      <DrawerContent>
        <DrawerHeader className="gap-1">
          <DrawerTitle className="font-mono">{item.name}</DrawerTitle>
          <DrawerDescription>
            {
              INVENTORY_GROUP_LABELS[
                item.group as keyof typeof INVENTORY_GROUP_LABELS
              ]
            }{" "}
            · {item.kind} · {item.node}
          </DrawerDescription>
        </DrawerHeader>
        <div className="flex flex-col gap-4 overflow-y-auto px-4 text-sm">
          {item.kind === "actuator" && !isMobile && (
            <>
              <ChartContainer config={chartConfig}>
                <AreaChart
                  accessibilityLayer
                  data={chartData}
                  margin={{
                    left: 0,
                    right: 10,
                  }}
                >
                  <CartesianGrid vertical={false} />
                  <XAxis
                    dataKey="sample"
                    tickLine={false}
                    axisLine={false}
                    tickMargin={8}
                    hide
                  />
                  <ChartTooltip
                    cursor={false}
                    content={<ChartTooltipContent indicator="dot" />}
                  />
                  <Area
                    dataKey="measured"
                    type="natural"
                    fill="var(--color-measured)"
                    fillOpacity={0.6}
                    stroke="var(--color-measured)"
                    stackId="a"
                  />
                  <Area
                    dataKey="commanded"
                    type="natural"
                    fill="var(--color-commanded)"
                    fillOpacity={0.4}
                    stroke="var(--color-commanded)"
                    stackId="a"
                  />
                </AreaChart>
              </ChartContainer>
              <Separator />
              <div className="grid gap-2">
                <div className="flex gap-2 leading-none font-medium">
                  Max tracking error 0.06 Nm this session{" "}
                  <HugeiconsIcon
                    icon={ChartUpIcon}
                    strokeWidth={2}
                    className="size-4"
                  />
                </div>
                <div className="text-muted-foreground">
                  Dummy telemetry for layout. Will sync to live CAN feedback and
                  time cursor when Chappe stream lands.
                </div>
              </div>
              <Separator />
            </>
          )}
          <form className="flex flex-col gap-4">
            <div className="flex flex-col gap-3">
              <Label htmlFor="name">Name</Label>
              <Input id="name" defaultValue={item.name} />
            </div>
            <div className="grid grid-cols-2 gap-4">
              <div className="flex flex-col gap-3">
                <Label htmlFor="kind">Kind</Label>
                <Select
                  defaultValue={item.kind}
                  items={[
                    { label: "actuator", value: "actuator" },
                    { label: "sensor", value: "sensor" },
                    { label: "device", value: "device" },
                  ]}
                >
                  <SelectTrigger id="kind" className="w-full">
                    <SelectValue placeholder="Select kind" />
                  </SelectTrigger>
                  <SelectContent>
                    <SelectGroup>
                      <SelectItem value="actuator">actuator</SelectItem>
                      <SelectItem value="sensor">sensor</SelectItem>
                      <SelectItem value="device">device</SelectItem>
                    </SelectGroup>
                  </SelectContent>
                </Select>
              </div>
              <div className="flex flex-col gap-3">
                <Label htmlFor="status">Status</Label>
                <Select
                  defaultValue={item.status}
                  items={[
                    { label: "Enabled", value: "Enabled" },
                    { label: "Nominal", value: "Nominal" },
                    { label: "Tuning", value: "Tuning" },
                    { label: "Fault", value: "Fault" },
                    { label: "Offline", value: "Offline" },
                  ]}
                >
                  <SelectTrigger id="status" className="w-full">
                    <SelectValue placeholder="Select status" />
                  </SelectTrigger>
                  <SelectContent>
                    <SelectGroup>
                      <SelectItem value="Enabled">Enabled</SelectItem>
                      <SelectItem value="Nominal">Nominal</SelectItem>
                      <SelectItem value="Tuning">Tuning</SelectItem>
                      <SelectItem value="Fault">Fault</SelectItem>
                      <SelectItem value="Offline">Offline</SelectItem>
                    </SelectGroup>
                  </SelectContent>
                </Select>
              </div>
            </div>
            <div className="grid grid-cols-2 gap-4">
              <div className="flex flex-col gap-3">
                <Label htmlFor="value">Reading</Label>
                <Input id="value" defaultValue={item.value} />
              </div>
              <div className="flex flex-col gap-3">
                <Label htmlFor="limit">Range</Label>
                <Input id="limit" defaultValue={item.limit} />
              </div>
            </div>
            <div className="flex flex-col gap-3">
              <Label htmlFor="preset">Preset</Label>
              <Select
                defaultValue={item.preset}
                items={[
                  { label: "golden_pose", value: "golden_pose" },
                  { label: "bench_default", value: "bench_default" },
                  { label: "tuning_sweep", value: "tuning_sweep" },
                  { label: "last_session", value: "last_session" },
                  { label: "unassigned", value: "unassigned" },
                ]}
              >
                <SelectTrigger id="preset" className="w-full">
                  <SelectValue placeholder="Select preset" />
                </SelectTrigger>
                <SelectContent>
                  <SelectGroup>
                    <SelectItem value="golden_pose">golden_pose</SelectItem>
                    <SelectItem value="bench_default">bench_default</SelectItem>
                    <SelectItem value="tuning_sweep">tuning_sweep</SelectItem>
                    <SelectItem value="last_session">last_session</SelectItem>
                    <SelectItem value="unassigned">unassigned</SelectItem>
                  </SelectGroup>
                </SelectContent>
              </Select>
            </div>
          </form>
        </div>
        <DrawerFooter>
          <Button>Apply</Button>
          <DrawerClose render={<Button variant="outline" />}></DrawerClose>
        </DrawerFooter>
      </DrawerContent>
    </Drawer>
  );
}
