import { cn } from '@/lib/utils';

type UsageBarProps = {
  value: number;
  className?: string;
};

function getFillClass(value: number): string {
  if (value >= 85) {
    return 'bg-amber-500';
  }

  if (value >= 70) {
    return 'bg-primary/90';
  }

  return 'bg-primary';
}

export function UsageBar({ value, className }: UsageBarProps) {
  const clamped = Math.min(100, Math.max(0, value));

  return (
    <div
      className={cn('h-1.5 w-full overflow-hidden rounded-full bg-muted/80', className)}
      role="progressbar"
      aria-valuenow={clamped}
      aria-valuemin={0}
      aria-valuemax={100}
    >
      <div
        className={cn('h-full rounded-full transition-[width]', getFillClass(clamped))}
        style={{ width: `${clamped}%` }}
      />
    </div>
  );
}
