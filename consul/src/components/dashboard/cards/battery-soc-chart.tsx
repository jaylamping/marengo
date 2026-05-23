import { PolarAngleAxis, RadialBar, RadialBarChart } from 'recharts';

import type { BatteryPack } from '@/data/battery-metrics';
import {
  ChartContainer,
  type ChartConfig,
} from '@/components/ui/chart';
import { cn } from '@/lib/utils';

function buildBatteryChartConfig(packs: BatteryPack[]): ChartConfig {
  const colors = ['var(--chart-1)', 'var(--chart-2)', 'var(--chart-3)'];

  return Object.fromEntries(
    packs.map((pack, index) => [
      pack.id,
      {
        label: pack.label,
        color: colors[index % colors.length],
      },
    ]),
  );
}

function buildBatteryChartData(packs: BatteryPack[]) {
  return [...packs]
    .sort((left, right) => {
      if (left.role === right.role) {
        return left.label.localeCompare(right.label);
      }

      return left.role === 'primary' ? -1 : 1;
    })
    .map((pack) => ({
      id: pack.id,
      soc: pack.socPercent,
      fill: `var(--color-${pack.id})`,
    }));
}

type BatterySocChartProps = {
  packs: BatteryPack[];
  aggregateSocPercent: number;
  className?: string;
};

export function BatterySocChart({
  packs,
  aggregateSocPercent,
  className,
}: BatterySocChartProps) {
  const chartConfig = buildBatteryChartConfig(packs);
  const chartData = buildBatteryChartData(packs);

  return (
    <div className={cn('relative', className)}>
      <ChartContainer
        config={chartConfig}
        className="mx-auto aspect-square max-h-[168px] w-full"
        initialDimension={{ width: 168, height: 168 }}
      >
        <RadialBarChart
          data={chartData}
          innerRadius={34}
          outerRadius={74}
          startAngle={90}
          endAngle={-270}
          barSize={8}
        >
          <PolarAngleAxis type="number" domain={[0, 100]} tick={false} axisLine={false} />
          <RadialBar
            dataKey="soc"
            background
            cornerRadius={4}
            strokeWidth={0}
          />
        </RadialBarChart>
      </ChartContainer>

      <div className="pointer-events-none absolute inset-0 flex flex-col items-center justify-center">
        <span className="text-3xl font-semibold tabular-nums tracking-tight">
          {aggregateSocPercent}%
        </span>
        <span className="text-xs text-muted-foreground">system SOC</span>
      </div>
    </div>
  );
}

type BatteryPackLegendProps = {
  packs: BatteryPack[];
};

export function BatteryPackLegend({ packs }: BatteryPackLegendProps) {
  const chartConfig = buildBatteryChartConfig(packs);

  return (
    <div className="grid gap-2">
      {packs.map((pack) => (
        <div key={pack.id} className="flex items-center justify-between gap-3 text-xs">
          <div className="flex min-w-0 items-center gap-2">
            <span
              className="size-2 shrink-0 rounded-full"
              style={{ backgroundColor: chartConfig[pack.id]?.color }}
            />
            <span className="truncate">{pack.label}</span>
          </div>
          <span className="font-mono tabular-nums text-muted-foreground">
            {pack.socPercent}% · {pack.sohPercent}% SOH
          </span>
        </div>
      ))}
    </div>
  );
}
