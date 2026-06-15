import { useEffect } from 'react';
import { useReducedMotion, useSpring } from 'motion/react';

/** Spring tuned for ~100ms Chappe telemetry — chases targets without restart jitter. */
const TELEMETRY_SPRING = {
  stiffness: 90,
  damping: 22,
  mass: 0.8,
} as const;

const REDUCED_MOTION_SPRING = {
  stiffness: 1000,
  damping: 100,
  mass: 1,
} as const;

export function useTelemetrySpring(target: number) {
  const reducedMotion = useReducedMotion();
  const spring = useSpring(target, {
    ...(reducedMotion ? REDUCED_MOTION_SPRING : TELEMETRY_SPRING),
  });

  useEffect(() => {
    spring.set(target);
  }, [spring, target]);

  return spring;
}
