import { useEffect, useState, type ReactNode } from 'react';

type DeferredMountProps = {
  children: ReactNode;
  fallback: ReactNode;
  /**
   * Max wait before mounting.
   * - strategy=paint: safety timeout after rAF (default 50)
   * - strategy=idle: requestIdleCallback timeout (default 2000)
   */
  timeoutMs?: number;
  /**
   * paint — next frames (route shells: skeleton then body).
   * idle — requestIdleCallback (heavy widgets that can wait).
   */
  strategy?: 'paint' | 'idle';
};

/**
 * Delay mounting children so the fallback can paint first.
 * Use strategy=paint for route bodies; strategy=idle for expensive widgets.
 */
export function DeferredMount({
  children,
  fallback,
  timeoutMs,
  strategy = 'paint',
}: DeferredMountProps) {
  const [mounted, setMounted] = useState(false);
  const resolvedTimeout = timeoutMs ?? (strategy === 'idle' ? 2000 : 50);

  useEffect(() => {
    let cancelled = false;
    const mount = () => {
      if (!cancelled) {
        setMounted(true);
      }
    };

    if (strategy === 'idle') {
      if (typeof requestIdleCallback === 'undefined') {
        const id = window.setTimeout(mount, 0);
        return () => {
          cancelled = true;
          window.clearTimeout(id);
        };
      }
      const id = requestIdleCallback(mount, { timeout: resolvedTimeout });
      return () => {
        cancelled = true;
        cancelIdleCallback(id);
      };
    }

    let raf2 = 0;
    const raf1 = window.requestAnimationFrame(() => {
      raf2 = window.requestAnimationFrame(mount);
    });
    const timeout = window.setTimeout(mount, resolvedTimeout);
    return () => {
      cancelled = true;
      window.cancelAnimationFrame(raf1);
      window.cancelAnimationFrame(raf2);
      window.clearTimeout(timeout);
    };
  }, [resolvedTimeout, strategy]);

  if (!mounted) {
    return fallback;
  }

  return children;
}
