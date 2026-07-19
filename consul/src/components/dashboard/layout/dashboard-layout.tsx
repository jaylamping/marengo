import type { ReactNode } from 'react';

import {
  dashboardChromeClassName,
  dashboardLayoutRootClassName,
  dashboardLayoutStyle,
  dashboardMainClassName,
  dashboardMainPointerClassName,
} from '@/components/dashboard/layout/constants';
import { SceneBackground } from '@/components/dashboard/layout/scene-background';
import { AppSidebar } from '@/components/dashboard/sidebar/app-sidebar';
import { SiteHeader } from '@/components/dashboard/site-header/site-header';
import { SidebarInset, SidebarProvider } from '@/components/ui/sidebar';

type DashboardLayoutProps = {
  children: ReactNode;
  /** Pause ambient dust WebGL during route transitions. */
  scenePaused?: boolean;
};

export function DashboardLayout({ children, scenePaused = false }: DashboardLayoutProps) {
  return (
    <div
      data-testid="dashboard-layout-root"
      className={dashboardLayoutRootClassName}
    >
      <SceneBackground paused={scenePaused} />
      <div className="app-vignette" aria-hidden />
      <SidebarProvider
        style={dashboardLayoutStyle}
        className={dashboardChromeClassName}
      >
        <AppSidebar variant="inset" />
        <SidebarInset className="bg-transparent">
          <SiteHeader />
          <div className="flex min-h-0 flex-1 flex-col">
            <div
              data-testid="dashboard-main"
              className={`${dashboardMainClassName} ${dashboardMainPointerClassName} min-h-0`}
            >
              {children}
            </div>
          </div>
        </SidebarInset>
      </SidebarProvider>
    </div>
  );
}
