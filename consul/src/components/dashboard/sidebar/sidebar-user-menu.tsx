import type { SidebarUser } from '@/data/sidebar-nav';
import {
  Avatar,
  AvatarFallback,
  AvatarImage,
} from '@/components/ui/avatar';
import {
  SidebarMenu,
  SidebarMenuItem,
} from '@/components/ui/sidebar';
import { SidebarUpdateStatus } from '@/components/dashboard/sidebar/sidebar-update-status';

type SidebarUserMenuProps = {
  user: SidebarUser;
};

function getInitials(name: string): string {
  return name
    .split(' ')
    .map((part) => part[0]?.toUpperCase() ?? '')
    .join('')
    .slice(0, 2);
}

/** Bench identity + deploy status / self-update controls. */
export function SidebarUserMenu({ user }: SidebarUserMenuProps) {
  const initials = getInitials(user.name);

  return (
    <SidebarMenu>
      <SidebarMenuItem>
        <div className="flex w-full flex-col">
          <div className="flex w-full items-center gap-2 rounded-md px-2 py-1.5">
            <Avatar className="size-8 rounded-lg grayscale">
              <AvatarImage src={user.avatar} alt={user.name} />
              <AvatarFallback className="rounded-lg">{initials}</AvatarFallback>
            </Avatar>
            <div className="grid min-w-0 flex-1 text-left text-sm leading-tight">
              <span className="truncate font-medium">{user.name}</span>
              <span className="truncate font-mono text-xs text-muted-foreground">
                {user.context}
              </span>
            </div>
          </div>
          <SidebarUpdateStatus />
        </div>
      </SidebarMenuItem>
    </SidebarMenu>
  );
}
