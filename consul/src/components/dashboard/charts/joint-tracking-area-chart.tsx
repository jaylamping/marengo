import { Area, AreaChart, CartesianGrid, XAxis } from 'recharts';

import { jointTrackingChartConfig } from '@/components/dashboard/charts/constants';
import type { JointTrackingPoint } from '@/components/dashboard/charts/types';
import {
  ChartContainer,
  ChartTooltip,
  ChartTooltipContent,
} from '@/components/ui/chart';

type JointTrackingAreaChartProps = {
  data: JointTrackingPoint[];
};

export function JointTrackingAreaChart({ data }: JointTrackingAreaChartProps) {
  return (
    <ChartContainer
      config={jointTrackingChartConfig}
      className="aspect-auto h-[250px] w-full"
    >
      <AreaChart data={data}>
        <defs>
          <linearGradient id="fillCommanded" x1="0" y1="0" x2="0" y2="1">
            <stop offset="5%" stopColor="var(--color-commanded)" stopOpacity={1.0} />
            <stop offset="95%" stopColor="var(--color-commanded)" stopOpacity={0.1} />
          </linearGradient>
          <linearGradient id="fillMeasured" x1="0" y1="0" x2="0" y2="1">
            <stop offset="5%" stopColor="var(--color-measured)" stopOpacity={0.8} />
            <stop offset="95%" stopColor="var(--color-measured)" stopOpacity={0.1} />
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
  );
}
