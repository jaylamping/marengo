import { Suspense, type ReactNode } from 'react';

import { DeferredMount } from '@/components/dashboard/layout/deferred-mount';
import { RouteBodyFallback } from '@/components/dashboard/layout/route-body-fallback';

type DeferredLazyBodyProps = {
  children: ReactNode;
  fallback?: ReactNode;
  timeoutMs?: number;
};

/** Paint-first route body: DeferredMount + Suspense with a shared skeleton. */
export function DeferredLazyBody({
  children,
  fallback = <RouteBodyFallback />,
  timeoutMs = 50,
}: DeferredLazyBodyProps) {
  return (
    <DeferredMount fallback={fallback} timeoutMs={timeoutMs}>
      <Suspense fallback={fallback}>{children}</Suspense>
    </DeferredMount>
  );
}
