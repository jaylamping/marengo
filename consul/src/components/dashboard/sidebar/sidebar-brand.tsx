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
          <HugeiconsIcon icon={CommandIcon} strokeWidth={2} className="size-5! text-accent" />
          <span className="font-mono text-sm font-medium uppercase tracking-[0.18em]">
            Consul
          </span>
        </SidebarMenuButton>
      </SidebarMenuItem>
    </SidebarMenu>
  );
}
