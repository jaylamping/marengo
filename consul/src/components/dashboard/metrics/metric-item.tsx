import { UsageBar } from '@/components/dashboard/metrics/usage-bar';
import { cn } from '@/lib/utils';

type MetricItemProps = {
  label: string;
  value: string;
  valueClassName?: string;
  usagePercent?: number;
};

export function MetricItem({
  label,
  value,
  valueClassName,
  usagePercent,
}: MetricItemProps) {
  return (
    <div>
      <div className="flex items-baseline justify-between gap-2">
        <dt className="text-xs text-muted-foreground">{label}</dt>
        <dd
          className={cn(
            'font-mono font-semibold tabular-nums',
            valueClassName ?? 'text-sm',
          )}
        >
          {value}
        </dd>
      </div>
      {usagePercent !== undefined ? (
        <UsageBar value={usagePercent} className="mt-1.5" />
      ) : null}
    </div>
  );
}
