import { Suspense } from 'react';
import { Outlet } from 'react-router-dom';

import { PageLoadingFallback } from '@/components/dashboard/layout/page-loading-fallback';

export function RootLayout() {
  return (
    <Suspense fallback={<PageLoadingFallback />}>
      <Outlet />
    </Suspense>
  );
}
