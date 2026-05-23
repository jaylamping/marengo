import type { ReactNode } from 'react';

import { dashboardLayoutStyle, dashboardMainClassName } from '@/components/dashboard/layout/constants';
import { AppSidebar } from '@/components/dashboard/sidebar/app-sidebar';
import { SiteHeader } from '@/components/dashboard/site-header/site-header';
import { SidebarInset, SidebarProvider } from '@/components/ui/sidebar';

type DashboardLayoutProps = {
  children: ReactNode;
};

export function DashboardLayout({ children }: DashboardLayoutProps) {
  return (
    <SidebarProvider style={dashboardLayoutStyle} className="min-h-svh">
      <AppSidebar variant="inset" />
      <SidebarInset>
        <SiteHeader />
        <div className="flex min-h-0 flex-1 flex-col">
          <div className={`${dashboardMainClassName} min-h-0`}>{children}</div>
        </div>
      </SidebarInset>
    </SidebarProvider>
  );
}
