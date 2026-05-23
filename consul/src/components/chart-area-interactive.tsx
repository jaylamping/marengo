'use client';

import * as React from 'react';
import { Area, AreaChart, CartesianGrid, XAxis } from 'recharts';

import { useIsMobile } from '@/hooks/use-mobile';
import {
  Card,
  CardAction,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from '@/components/ui/card';
import {
  ChartContainer,
  ChartTooltip,
  ChartTooltipContent,
  type ChartConfig,
} from '@/components/ui/chart';
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from '@/components/ui/select';
import {
  ToggleGroup,
  ToggleGroupItem,
} from '@/components/ui/toggle-group';

export const description = 'Joint tracking error over session time';

const chartData = [
  { time: '00:00', commanded: 0.42, measured: 0.38 },
  { time: '00:05', commanded: 0.55, measured: 0.51 },
  { time: '00:10', commanded: 0.61, measured: 0.58 },
  { time: '00:15', commanded: 0.48, measured: 0.44 },
  { time: '00:20', commanded: 0.72, measured: 0.69 },
  { time: '00:25', commanded: 0.66, measured: 0.62 },
  { time: '00:30', commanded: 0.58, measured: 0.55 },
  { time: '00:35', commanded: 0.81, measured: 0.77 },
  { time: '00:40', commanded: 0.74, measured: 0.71 },
  { time: '00:45', commanded: 0.69, measured: 0.64 },
  { time: '00:50', commanded: 0.92, measured: 0.88 },
  { time: '00:55', commanded: 0.85, measured: 0.79 },
  { time: '01:00', commanded: 0.78, measured: 0.73 },
  { time: '01:05', commanded: 0.63, measured: 0.59 },
  { time: '01:10', commanded: 0.57, measured: 0.52 },
  { time: '01:15', commanded: 0.49, measured: 0.45 },
  { time: '01:20', commanded: 0.71, measured: 0.67 },
  { time: '01:25', commanded: 0.88, measured: 0.84 },
  { time: '01:30', commanded: 0.94, measured: 0.89 },
  { time: '01:35', commanded: 0.86, measured: 0.81 },
  { time: '01:40', commanded: 0.79, measured: 0.74 },
  { time: '01:45', commanded: 0.68, measured: 0.63 },
  { time: '01:50', commanded: 0.59, measured: 0.54 },
  { time: '01:55', commanded: 0.52, measured: 0.48 },
  { time: '02:00', commanded: 0.47, measured: 0.43 },
];

const chartConfig = {
  tracking: {
    label: 'Tracking',
  },
  commanded: {
    label: 'Commanded (Nm)',
    color: 'var(--primary)',
  },
  measured: {
    label: 'Measured (Nm)',
    color: 'var(--chart-2)',
  },
} satisfies ChartConfig;

export function ChartAreaInteractive() {
  const isMobile = useIsMobile();
  const [timeRange, setTimeRange] = React.useState('session');

  React.useEffect(() => {
    if (isMobile) {
      setTimeRange('1m');
    }
  }, [isMobile]);

  const filteredData = chartData.filter((item) => {
    const [, seconds] = item.time.split(':').map(Number);
    if (timeRange === 'session') {
      return true;
    }
    if (timeRange === '5m') {
      return seconds <= 55 || item.time.startsWith('00:');
    }
    return seconds >= 55 || item.time.startsWith('01:') || item.time === '02:00';
  });

  return (
    <Card className="@container/card">
      <CardHeader>
        <CardTitle>shoulder_pitch · torque tracking</CardTitle>
        <CardDescription>
          <span className="hidden @[540px]/card:block">
            Commanded vs measured torque for current session
          </span>
          <span className="@[540px]/card:hidden">Torque tracking</span>
        </CardDescription>
        <CardAction>
          <ToggleGroup
            multiple={false}
            value={timeRange ? [timeRange] : []}
            onValueChange={(value) => {
              setTimeRange(value[0] ?? 'session');
            }}
            variant="outline"
            className="hidden *:data-[slot=toggle-group-item]:px-4! @[767px]/card:flex"
          >
            <ToggleGroupItem value="session">Session</ToggleGroupItem>
            <ToggleGroupItem value="5m">Last 5 min</ToggleGroupItem>
            <ToggleGroupItem value="1m">Last 1 min</ToggleGroupItem>
          </ToggleGroup>
          <Select
            value={timeRange}
            onValueChange={(value) => {
              if (value !== null) {
                setTimeRange(value);
              }
            }}
          >
            <SelectTrigger
              className="flex w-40 **:data-[slot=select-value]:block **:data-[slot=select-value]:truncate @[767px]/card:hidden"
              size="sm"
              aria-label="Select time range"
            >
              <SelectValue placeholder="Session" />
            </SelectTrigger>
            <SelectContent className="rounded-xl">
              <SelectItem value="session" className="rounded-lg">
                Session
              </SelectItem>
              <SelectItem value="5m" className="rounded-lg">
                Last 5 min
              </SelectItem>
              <SelectItem value="1m" className="rounded-lg">
                Last 1 min
              </SelectItem>
            </SelectContent>
          </Select>
        </CardAction>
      </CardHeader>
      <CardContent className="px-2 pt-4 sm:px-6 sm:pt-6">
        <ChartContainer
          config={chartConfig}
          className="aspect-auto h-[250px] w-full"
        >
          <AreaChart data={filteredData}>
            <defs>
              <linearGradient id="fillCommanded" x1="0" y1="0" x2="0" y2="1">
                <stop
                  offset="5%"
                  stopColor="var(--color-commanded)"
                  stopOpacity={1.0}
                />
                <stop
                  offset="95%"
                  stopColor="var(--color-commanded)"
                  stopOpacity={0.1}
                />
              </linearGradient>
              <linearGradient id="fillMeasured" x1="0" y1="0" x2="0" y2="1">
                <stop
                  offset="5%"
                  stopColor="var(--color-measured)"
                  stopOpacity={0.8}
                />
                <stop
                  offset="95%"
                  stopColor="var(--color-measured)"
                  stopOpacity={0.1}
                />
              </linearGradient>
            </defs>
            <CartesianGrid vertical={false} />
            <XAxis
              dataKey="time"
              tickLine={false}
              axisLine={false}
              tickMargin={8}
              minTickGap={32}
            />
            <ChartTooltip
              cursor={false}
              content={
                <ChartTooltipContent
                  labelFormatter={(value) => `t+${value}`}
                  indicator="dot"
                />
              }
            />
            <Area
              dataKey="measured"
              type="natural"
              fill="url(#fillMeasured)"
              stroke="var(--color-measured)"
              stackId="a"
            />
            <Area
              dataKey="commanded"
              type="natural"
              fill="url(#fillCommanded)"
              stroke="var(--color-commanded)"
              stackId="a"
            />
          </AreaChart>
        </ChartContainer>
      </CardContent>
    </Card>
  );
}
