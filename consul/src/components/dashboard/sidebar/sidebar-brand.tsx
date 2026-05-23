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
        <SidebarMenuButton
          className="data-[slot=sidebar-menu-button]:p-1.5!"
          render={<a href="#" />}
        >
          <HugeiconsIcon icon={CommandIcon} strokeWidth={2} className="size-5!" />
          <span className="text-base font-semibold">Consul</span>
        </SidebarMenuButton>
      </SidebarMenuItem>
    </SidebarMenu>
  );
}
