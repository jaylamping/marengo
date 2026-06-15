import { useMotionValueEvent } from 'motion/react';
import { useState } from 'react';

import { useTelemetrySpring } from '@/hooks/use-telemetry-spring';
import { cn } from '@/lib/utils';

type AnimatedNumberProps = {
  value: number;
  format: (value: number) => string;
  className?: string;
};

export function AnimatedNumber({ value, format, className }: AnimatedNumberProps) {
  const spring = useTelemetrySpring(value);
  const [display, setDisplay] = useState(() => format(value));

  useMotionValueEvent(spring, 'change', (latest) => {
    setDisplay(format(latest));
  });

  return (
    <span className={cn('font-mono font-semibold tabular-nums', className)}>
      {display}
    </span>
  );
}
