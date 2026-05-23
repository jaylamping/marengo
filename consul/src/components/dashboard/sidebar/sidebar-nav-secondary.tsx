import type { ComponentPropsWithoutRef } from 'react';

import type { SidebarNavItem } from '@/data/sidebar-nav';
import { SidebarIcon } from '@/components/dashboard/sidebar/sidebar-icon';
import {
  SidebarGroup,
  SidebarGroupContent,
  SidebarMenu,
  SidebarMenuButton,
  SidebarMenuItem,
} from '@/components/ui/sidebar';

type SidebarNavSecondaryProps = {
  items: SidebarNavItem[];
} & ComponentPropsWithoutRef<typeof SidebarGroup>;

export function SidebarNavSecondary({
  items,
  ...props
}: SidebarNavSecondaryProps) {
  return (
    <SidebarGroup {...props}>
      <SidebarGroupContent>
        <SidebarMenu>
          {items.map((item) => (
            <SidebarMenuItem key={item.title}>
              <SidebarMenuButton render={<a href={item.url} />}>
                <SidebarIcon icon={item.icon} />
                <span>{item.title}</span>
              </SidebarMenuButton>
            </SidebarMenuItem>
          ))}
        </SidebarMenu>
      </SidebarGroupContent>
    </SidebarGroup>
  );
}
