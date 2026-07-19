import { NavLink } from 'react-router-dom';

import {
  SidebarMenu,
  SidebarMenuButton,
  SidebarMenuItem,
} from '@/components/ui/sidebar';
import { HugeiconsIcon } from '@hugeicons/react';
import { CommandIcon } from '@hugeicons/core-free-icons';

export function SidebarBrand() {
  return (
    <SidebarMenu>
      <SidebarMenuItem>
        <NavLink to="/" end>
          {({ isActive }) => (
            <SidebarMenuButton
              isActive={isActive}
              className="data-[slot=sidebar-menu-button]:p-1.5!"
            >
              <HugeiconsIcon icon={CommandIcon} strokeWidth={2} className="size-5! text-accent" />
              <span className="font-mono text-sm font-medium uppercase tracking-[0.18em]">
                Consul
              </span>
            </SidebarMenuButton>
          )}
        </NavLink>
      </SidebarMenuItem>
    </SidebarMenu>
  );
}
