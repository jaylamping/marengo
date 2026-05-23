import type { BatteryPack } from '@/data/battery-metrics';
import { cn } from '@/lib/utils';

const PACK_COLORS = ['var(--chart-1)', 'var(--chart-2)', 'var(--chart-3)'] as const;

const RING_STROKE = 8;
const RING_GAP = 10;
const VIEW_SIZE = 168;
const CENTER = VIEW_SIZE / 2;
const OUTER_RADIUS = 74;

function getPackColor(index: number): string {
  return PACK_COLORS[index % PACK_COLORS.length] ?? PACK_COLORS[0];
}

function sortPacksForRings(packs: BatteryPack[]): BatteryPack[] {
  return [...packs].sort((left, right) => {
    if (left.role === right.role) {
      return left.label.localeCompare(right.label);
    }

    return left.role === 'primary' ? -1 : 1;
  });
}

function ringRadius(index: number, ringCount: number): number {
  return OUTER_RADIUS - (ringCount - 1 - index) * (RING_STROKE + RING_GAP);
}

type BatterySocRingsProps = {
  packs: BatteryPack[];
  aggregateSocPercent: number;
  className?: string;
};

export function BatterySocRings({
  packs,
  aggregateSocPercent,
  className,
}: BatterySocRingsProps) {
  const sortedPacks = sortPacksForRings(packs);

  return (
    <div className={cn('relative mx-auto max-w-[180px]', className)}>
      <svg
        viewBox={`0 0 ${VIEW_SIZE} ${VIEW_SIZE}`}
        className="size-full"
        role="img"
        aria-label={`Battery system state of charge ${aggregateSocPercent} percent`}
      >
        {sortedPacks.map((pack, index) => {
          const radius = ringRadius(index, sortedPacks.length);
          const circumference = 2 * Math.PI * radius;
          const dash = (pack.socPercent / 100) * circumference;

          return (
            <g key={pack.id}>
              <circle
                cx={CENTER}
                cy={CENTER}
                r={radius}
                fill="none"
                stroke="currentColor"
                strokeWidth={RING_STROKE}
                className="text-muted/50"
              />
              <circle
                cx={CENTER}
                cy={CENTER}
                r={radius}
                fill="none"
                stroke={getPackColor(index)}
                strokeWidth={RING_STROKE}
                strokeLinecap="round"
                strokeDasharray={`${dash} ${circumference - dash}`}
                transform={`rotate(-90 ${CENTER} ${CENTER})`}
              />
            </g>
          );
        })}
      </svg>

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
  const sortedPacks = sortPacksForRings(packs);

  return (
    <div className="grid gap-2">
      {sortedPacks.map((pack, index) => (
        <div key={pack.id} className="flex items-center justify-between gap-3 text-xs">
          <div className="flex min-w-0 items-center gap-2">
            <span
              className="size-2 shrink-0 rounded-full"
              style={{ backgroundColor: getPackColor(index) }}
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
