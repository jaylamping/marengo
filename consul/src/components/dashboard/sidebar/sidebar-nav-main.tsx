import { NavLink } from 'react-router-dom';

import type { SidebarNavItem } from '@/data/sidebar-nav';
import { SidebarIcon } from '@/components/dashboard/sidebar/sidebar-icon';
import {
  SidebarGroup,
  SidebarGroupContent,
  SidebarMenu,
  SidebarMenuButton,
  SidebarMenuItem,
} from '@/components/ui/sidebar';
import { cn } from '@/lib/utils';

type SidebarNavMainProps = {
  items: SidebarNavItem[];
};

function isRoutedItem(url: string) {
  return url.startsWith('/');
}

export function SidebarNavMain({ items }: SidebarNavMainProps) {
  return (
    <SidebarGroup>
      <SidebarGroupContent className="flex flex-col gap-2">
        <SidebarMenu>
          {items.map((item) => (
            <SidebarMenuItem key={item.title}>
              {isRoutedItem(item.url) ? (
                <NavLink to={item.url} end={item.url === '/'}>
                  {({ isActive }) => (
                    <SidebarMenuButton
                      tooltip={item.title}
                      isActive={isActive}
                      className={cn(
                        isActive &&
                          'bg-surface-2 text-accent shadow-[inset_2px_0_0_var(--accent)] hover:bg-surface-2 hover:text-accent active:bg-surface-2 active:text-accent',
                      )}
                    >
                      <SidebarIcon icon={item.icon} />
                      <span>{item.title}</span>
                    </SidebarMenuButton>
                  )}
                </NavLink>
              ) : (
                <SidebarMenuButton tooltip={item.title} render={<a href={item.url} />}>
                  <SidebarIcon icon={item.icon} />
                  <span>{item.title}</span>
                </SidebarMenuButton>
              )}
            </SidebarMenuItem>
          ))}
        </SidebarMenu>
      </SidebarGroupContent>
    </SidebarGroup>
  );
}
