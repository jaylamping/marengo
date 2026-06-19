import type { ComponentProps } from 'react';

import { SidebarBrand } from '@/components/dashboard/sidebar/sidebar-brand';
import { SidebarNavMain } from '@/components/dashboard/sidebar/sidebar-nav-main';
import { SidebarNavSecondary } from '@/components/dashboard/sidebar/sidebar-nav-secondary';
import { SidebarPresetsNav } from '@/components/dashboard/sidebar/sidebar-presets-nav';
import { SidebarUserMenu } from '@/components/dashboard/sidebar/sidebar-user-menu';
import {
  getSidebarNavMain,
  sidebarNavSecondary,
  sidebarPresets,
  sidebarUser,
} from '@/data/sidebar-nav';
import {
  Sidebar,
  SidebarContent,
  SidebarFooter,
  SidebarHeader,
} from '@/components/ui/sidebar';

export function AppSidebar(props: ComponentProps<typeof Sidebar>) {
  return (
    <Sidebar collapsible="offcanvas" {...props}>
      <SidebarHeader>
        <SidebarBrand />
      </SidebarHeader>
      <SidebarContent>
        <SidebarNavMain items={getSidebarNavMain()} />
        <SidebarPresetsNav items={sidebarPresets} />
        <SidebarNavSecondary items={sidebarNavSecondary} className="mt-auto" />
      </SidebarContent>
      <SidebarFooter>
        <SidebarUserMenu user={sidebarUser} />
      </SidebarFooter>
    </Sidebar>
  );
}
