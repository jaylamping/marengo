import { cn } from '@/lib/utils';

type MetricItemProps = {
  label: string;
  value: string;
  valueClassName?: string;
};

export function MetricItem({ label, value, valueClassName }: MetricItemProps) {
  return (
    <div>
      <dt className="text-xs text-muted-foreground">{label}</dt>
      <dd
        className={cn(
          'font-mono font-semibold tabular-nums',
          valueClassName ?? 'text-lg',
        )}
      >
        {value}
      </dd>
    </div>
  );
}
