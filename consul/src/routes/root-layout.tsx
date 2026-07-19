import { Suspense, lazy, useEffect, useState } from 'react';
import { Outlet, useLocation } from 'react-router-dom';

import { DashboardLayout } from '@/components/dashboard/layout/dashboard-layout';
import { PageLoadingFallback } from '@/components/dashboard/layout/page-loading-fallback';
import { prefetchHeavyRoutes } from '@/lib/prefetch-routes';

const DashboardPage = lazy(async () => {
  const module = await import('@/pages/dashboard');
  return { default: module.DashboardPage };
});

/**
 * Persistent chrome + SceneBackground across routes.
 * Overview (WebGL posture) soft-stays mounted briefly after leave so nav
 * paint is not blocked by Three.js teardown on the same frame.
 */
export function RootLayout() {
  const { pathname } = useLocation();
  const isOverview = pathname === '/';
  const [overviewCached, setOverviewCached] = useState(isOverview);
  const [scenePaused, setScenePaused] = useState(false);

  if (isOverview && !overviewCached) {
    setOverviewCached(true);
  }

  useEffect(() => {
    if (isOverview) {
      return;
    }
    const idle =
      typeof requestIdleCallback === 'undefined'
        ? undefined
        : requestIdleCallback(() => setOverviewCached(false), { timeout: 2000 });
    const fallback =
      idle === undefined
        ? window.setTimeout(() => setOverviewCached(false), 500)
        : undefined;
    return () => {
      if (idle !== undefined) {
        cancelIdleCallback(idle);
      }
      if (fallback !== undefined) {
        window.clearTimeout(fallback);
      }
    };
  }, [isOverview]);

  useEffect(() => {
    setScenePaused(true);
    const timer = window.setTimeout(() => setScenePaused(false), 280);
    return () => window.clearTimeout(timer);
  }, [pathname]);

  useEffect(() => {
    prefetchHeavyRoutes();
  }, []);

  return (
    <DashboardLayout scenePaused={scenePaused}>
      <Suspense fallback={<PageLoadingFallback />}>
        {overviewCached ? (
          <div className={isOverview ? 'contents' : 'hidden'} hidden={!isOverview}>
            <DashboardPage active={isOverview} />
          </div>
        ) : null}
        {!isOverview || !overviewCached ? <Outlet /> : null}
      </Suspense>
    </DashboardLayout>
  );
}
