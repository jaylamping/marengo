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
    <SidebarProvider style={dashboardLayoutStyle}>
      <AppSidebar variant="inset" />
      <SidebarInset>
        <SiteHeader />
        <div className="flex flex-1 flex-col">
          <div className={dashboardMainClassName}>{children}</div>
        </div>
      </SidebarInset>
    </SidebarProvider>
  );
}
