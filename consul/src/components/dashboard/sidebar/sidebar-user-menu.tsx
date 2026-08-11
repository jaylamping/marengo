import type { SidebarUser } from '@/data/sidebar-nav';
import {
  SidebarMenu,
  SidebarMenuItem,
} from '@/components/ui/sidebar';
import { SidebarUpdateStatus } from '@/components/dashboard/sidebar/sidebar-update-status';

type SidebarUserMenuProps = {
  user: SidebarUser;
};

/** Bench identity + deploy status / self-update controls. */
export function SidebarUserMenu({ user }: SidebarUserMenuProps) {
  return (
    <SidebarMenu>
      <SidebarMenuItem>
        <SidebarUpdateStatus user={user} />
      </SidebarMenuItem>
    </SidebarMenu>
  );
}
