import { AnimatedNumber } from '@/components/dashboard/metrics/animated-number';
import { UsageBar } from '@/components/dashboard/metrics/usage-bar';
import { cn } from '@/lib/utils';

type MetricItemProps = {
  label: string;
  value: string;
  valueClassName?: string;
  /** When set, the displayed value springs toward telemetry updates. */
  smoothValue?: number;
  formatSmoothValue?: (value: number) => string;
  usagePercent?: number;
};

export function MetricItem({
  label,
  value,
  valueClassName,
  smoothValue,
  formatSmoothValue,
  usagePercent,
}: MetricItemProps) {
  const valueClass = cn(
    'font-mono font-semibold tabular-nums',
    valueClassName ?? 'text-sm',
  );

  return (
    <div>
      <div className="flex items-baseline justify-between gap-2">
        <dt className="text-xs text-muted-foreground">{label}</dt>
        <dd>
          {smoothValue !== undefined && formatSmoothValue ? (
            <AnimatedNumber
              value={smoothValue}
              format={formatSmoothValue}
              className={valueClass}
            />
          ) : (
            <span className={valueClass}>{value}</span>
          )}
        </dd>
      </div>
      {usagePercent !== undefined ? (
        <UsageBar value={usagePercent} className="mt-1.5" />
      ) : null}
    </div>
  );
}
