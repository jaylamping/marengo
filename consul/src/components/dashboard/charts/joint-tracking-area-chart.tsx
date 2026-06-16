import { memo } from 'react';
import { Area, AreaChart, CartesianGrid, XAxis, ReferenceLine, ReferenceArea } from 'recharts';

import { jointTrackingChartConfig } from '@/components/dashboard/charts/constants';
import type { JointTrackingPoint, JointLimits, JointSafety } from '@/components/dashboard/charts/types';
import {
  ChartContainer,
  ChartTooltip,
  ChartTooltipContent,
} from '@/components/ui/chart';

type JointTrackingAreaChartProps = {
  data: JointTrackingPoint[];
  limits?: JointLimits | null;
  safety?: JointSafety | null;
};

export const JointTrackingAreaChart = memo(function JointTrackingAreaChart({
  data,
  limits,
  safety,
}: JointTrackingAreaChartProps) {
  return (
    <ChartContainer
      config={jointTrackingChartConfig}
      className="aspect-auto h-[250px] w-full min-h-[250px]"
      debounce={150}
    >
      <AreaChart data={data} margin={{ left: 12, right: 12 }}>
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
          isAnimationActive={false}
          content={
            <ChartTooltipContent
              labelFormatter={(value) => `t+${value}`}
              indicator="dot"
            />
          }
        />
        {safety && (
          <>
            <ReferenceArea
              y1={safety.softLower}
              y2={limits?.lower ?? safety.softLower}
              fill="var(--color-warning)"
              fillOpacity={0.15}
              stroke="none"
            />
            <ReferenceArea
              y1={limits?.upper ?? safety.softUpper}
              y2={safety.softUpper}
              fill="var(--color-warning)"
              fillOpacity={0.15}
              stroke="none"
            />
          </>
        )}
        {limits && (
          <>
            <ReferenceLine y={limits.lower} stroke="var(--color-destructive)" strokeDasharray="4 4" strokeWidth={1.5} />
            <ReferenceLine y={limits.upper} stroke="var(--color-destructive)" strokeDasharray="4 4" strokeWidth={1.5} />
          </>
        )}
        <Area
          dataKey="measured"
          type="monotone"
          fill="url(#fillMeasured)"
          stroke="var(--color-measured)"
          isAnimationActive={false}
          dot={false}
          activeDot={false}
        />
        <Area
          dataKey="commanded"
          type="monotone"
          fill="url(#fillCommanded)"
          stroke="var(--color-commanded)"
          isAnimationActive={false}
          dot={false}
          activeDot={false}
        />
      </AreaChart>
    </ChartContainer>
  );
});
