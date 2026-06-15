import { useEffect, useState } from 'react';

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
  const target = Math.min(100, Math.max(0, value));
  const [width, setWidth] = useState(0);

  useEffect(() => {
    const id = requestAnimationFrame(() => setWidth(target));
    return () => cancelAnimationFrame(id);
  }, [target]);

  return (
    <div
      className={cn('h-1.5 w-full overflow-hidden rounded-full bg-muted/80', className)}
      role="progressbar"
      aria-valuenow={target}
      aria-valuemin={0}
      aria-valuemax={100}
    >
      <div
        className={cn(
          'h-full rounded-full transition-[width] duration-500 ease-in-out motion-reduce:transition-none',
          getFillClass(target),
        )}
        style={{ width: `${width}%` }}
      />
    </div>
  );
}
