import { useEffect, useState, type ReactNode } from 'react';

type DeferredMountProps = {
  children: ReactNode;
  fallback: ReactNode;
  /** Max wait before mount even if the main thread stays busy. */
  timeoutMs?: number;
};

export function DeferredMount({
  children,
  fallback,
  timeoutMs = 2000,
}: DeferredMountProps) {
  const [mounted, setMounted] = useState(false);

  useEffect(() => {
    if (typeof requestIdleCallback === 'undefined') {
      const id = window.setTimeout(() => setMounted(true), 0);
      return () => window.clearTimeout(id);
    }

    const id = requestIdleCallback(() => setMounted(true), { timeout: timeoutMs });
    return () => cancelIdleCallback(id);
  }, [timeoutMs]);

  if (!mounted) {
    return fallback;
  }

  return children;
}
