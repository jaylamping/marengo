import { motion, useTransform } from 'motion/react';

import { useTelemetrySpring } from '@/hooks/use-telemetry-spring';
import { cn } from '@/lib/utils';

type UsageBarProps = {
  value: number;
  className?: string;
};

function getFillClass(value: number): string {
  if (value >= 85) {
    return 'bg-warning';
  }

  if (value >= 70) {
    return 'bg-primary/90';
  }

  return 'bg-primary';
}

export function UsageBar({ value, className }: UsageBarProps) {
  const target = Math.min(100, Math.max(0, value));
  const spring = useTelemetrySpring(target);
  const scaleX = useTransform(spring, (latest) => latest / 100);

  return (
    <div
      className={cn('h-1.5 w-full overflow-hidden rounded-full bg-muted/80', className)}
      role="progressbar"
      aria-valuenow={target}
      aria-valuemin={0}
      aria-valuemax={100}
    >
      <motion.div
        className={cn('h-full w-full origin-left rounded-full', getFillClass(target))}
        style={{ scaleX }}
      />
    </div>
  );
}
