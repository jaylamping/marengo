/** Trailing throttle: at most one call per window; last args win. */
export function throttleTrailing<T extends (...args: never[]) => void>(
  fn: T,
  minIntervalMs: number,
): (...args: Parameters<T>) => void {
  let lastRun = 0;
  let timer: number | undefined;
  let pending: Parameters<T> | undefined;

  const flush = () => {
    timer = undefined;
    if (!pending) {
      return;
    }
    const args = pending;
    pending = undefined;
    lastRun = Date.now();
    fn(...args);
  };

  return (...args: Parameters<T>) => {
    pending = args;
    const now = Date.now();
    const elapsed = now - lastRun;
    if (elapsed >= minIntervalMs) {
      if (timer !== undefined) {
        window.clearTimeout(timer);
        timer = undefined;
      }
      flush();
      return;
    }
    if (timer === undefined) {
      timer = window.setTimeout(flush, minIntervalMs - elapsed);
    }
  };
}

/** Trailing debounce: emit once after `waitMs` quiet time; last args win. */
export function debounceTrailing<T extends (...args: never[]) => void>(
  fn: T,
  waitMs: number,
): (...args: Parameters<T>) => void {
  let timer: number | undefined;
  return (...args: Parameters<T>) => {
    if (timer !== undefined) {
      window.clearTimeout(timer);
    }
    timer = window.setTimeout(() => {
      timer = undefined;
      fn(...args);
    }, waitMs);
  };
}
