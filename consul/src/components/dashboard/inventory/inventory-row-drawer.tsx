import { Area, AreaChart, CartesianGrid, XAxis } from 'recharts';

import {
  KIND_OPTIONS,
  PRESET_OPTIONS_WITH_UNASSIGNED,
  STATUS_OPTIONS,
  actuatorTrackingChartConfig,
  actuatorTrackingChartData,
  inventoryDrawerContentClassName,
} from '@/components/dashboard/inventory/constants';
import type { InventoryRow } from '@/components/dashboard/inventory/types';
import { useIsMobile } from '@/hooks/use-mobile';
import { INVENTORY_GROUP_LABELS } from '@/data/robot-inventory';
import { Button } from '@/components/ui/button';
import {
  ChartContainer,
  ChartTooltip,
  ChartTooltipContent,
} from '@/components/ui/chart';
import {
  Drawer,
  DrawerClose,
  DrawerContent,
  DrawerDescription,
  DrawerFooter,
  DrawerHeader,
  DrawerTitle,
  DrawerTrigger,
} from '@/components/ui/drawer';
import { Input } from '@/components/ui/input';
import { Label } from '@/components/ui/label';
import {
  Select,
  SelectContent,
  SelectGroup,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from '@/components/ui/select';
import { Separator } from '@/components/ui/separator';
import { HugeiconsIcon } from '@hugeicons/react';
import { ChartUpIcon } from '@hugeicons/core-free-icons';

type InventoryRowDrawerProps = {
  item: InventoryRow;
};

export function InventoryRowDrawer({ item }: InventoryRowDrawerProps) {
  const isMobile = useIsMobile();

  return (
    <Drawer direction={isMobile ? 'bottom' : 'right'}>
      <DrawerTrigger asChild>
        <Button
          variant="link"
          className="w-fit px-0 text-left font-mono text-sm text-foreground"
        >
          {item.name}
        </Button>
      </DrawerTrigger>
      <DrawerContent className={inventoryDrawerContentClassName}>
        <DrawerHeader className="gap-1">
          <DrawerTitle className="font-mono">{item.name}</DrawerTitle>
          <DrawerDescription>
            {INVENTORY_GROUP_LABELS[item.group]} · {item.kind} · {item.node}
          </DrawerDescription>
        </DrawerHeader>
        <div className="flex flex-col gap-4 overflow-y-auto px-4 text-sm">
          {item.kind === 'actuator' && !isMobile ? (
            <>
              <ChartContainer config={actuatorTrackingChartConfig}>
                <AreaChart
                  accessibilityLayer
                  data={actuatorTrackingChartData}
                  margin={{ left: 0, right: 10 }}
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
                  Max tracking error 0.06 Nm this session
                  <HugeiconsIcon icon={ChartUpIcon} strokeWidth={2} className="size-4" />
                </div>
                <div className="text-muted-foreground">
                  Dummy telemetry for layout. Will sync to live CAN feedback and
                  time cursor when Chappe stream lands.
                </div>
              </div>
              <Separator />
            </>
          ) : null}
          <form className="flex flex-col gap-4">
            <div className="flex flex-col gap-3">
              <Label htmlFor="name">Name</Label>
              <Input id="name" defaultValue={item.name} />
            </div>
            <div className="grid grid-cols-2 gap-4">
              <div className="flex flex-col gap-3">
                <Label htmlFor="kind">Kind</Label>
                <Select defaultValue={item.kind} items={[...KIND_OPTIONS]}>
                  <SelectTrigger id="kind" className="w-full">
                    <SelectValue placeholder="Select kind" />
                  </SelectTrigger>
                  <SelectContent>
                    <SelectGroup>
                      {KIND_OPTIONS.map((option) => (
                        <SelectItem key={option.value} value={option.value}>
                          {option.label}
                        </SelectItem>
                      ))}
                    </SelectGroup>
                  </SelectContent>
                </Select>
              </div>
              <div className="flex flex-col gap-3">
                <Label htmlFor="status">Status</Label>
                <Select defaultValue={item.status} items={[...STATUS_OPTIONS]}>
                  <SelectTrigger id="status" className="w-full">
                    <SelectValue placeholder="Select status" />
                  </SelectTrigger>
                  <SelectContent>
                    <SelectGroup>
                      {STATUS_OPTIONS.map((option) => (
                        <SelectItem key={option.value} value={option.value}>
                          {option.label}
                        </SelectItem>
                      ))}
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
                items={[...PRESET_OPTIONS_WITH_UNASSIGNED]}
              >
                <SelectTrigger id="preset" className="w-full">
                  <SelectValue placeholder="Select preset" />
                </SelectTrigger>
                <SelectContent>
                  <SelectGroup>
                    {PRESET_OPTIONS_WITH_UNASSIGNED.map((option) => (
                      <SelectItem key={option.value} value={option.value}>
                        {option.label}
                      </SelectItem>
                    ))}
                  </SelectGroup>
                </SelectContent>
              </Select>
            </div>
          </form>
        </div>
        <DrawerFooter>
          <Button>Apply</Button>
          <DrawerClose asChild>
            <Button variant="outline">Cancel</Button>
          </DrawerClose>
        </DrawerFooter>
      </DrawerContent>
    </Drawer>
  );
}
