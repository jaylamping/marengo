import { useEffect, useMemo, useState } from 'react';

import { throttleTrailing } from '@/lib/throttle-callback';

/** Trailing throttle for display-only values (e.g. live chart series). */
export function useThrottledValue<T>(value: T, minIntervalMs: number): T {
  const [display, setDisplay] = useState(value);
  const publish = useMemo(
    () => throttleTrailing((next: T) => setDisplay(next), minIntervalMs),
    [minIntervalMs],
  );

  useEffect(() => {
    publish(value);
  }, [value, publish]);

  return display;
}
