import { Suspense } from 'react';
import { Outlet } from 'react-router-dom';

import { DashboardLayout } from '@/components/dashboard/layout/dashboard-layout';
import { PageLoadingFallback } from '@/components/dashboard/layout/page-loading-fallback';

/**
 * Persistent chrome + SceneBackground across routes.
 * Suspense only wraps page body so WebGL dust and sidebar do not remount on nav.
 */
export function RootLayout() {
  return (
    <DashboardLayout>
      <Suspense fallback={<PageLoadingFallback />}>
        <Outlet />
      </Suspense>
    </DashboardLayout>
  );
}
