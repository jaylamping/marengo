import * as React from 'react';

import { NavDocuments } from '@/components/nav-documents';
import { NavMain } from '@/components/nav-main';
import { NavSecondary } from '@/components/nav-secondary';
import { NavUser } from '@/components/nav-user';
import {
  Sidebar,
  SidebarContent,
  SidebarFooter,
  SidebarHeader,
  SidebarMenu,
  SidebarMenuButton,
  SidebarMenuItem,
} from '@/components/ui/sidebar';
import { HugeiconsIcon } from '@hugeicons/react';
import {
  Analytics01Icon,
  ChartHistogramIcon,
  CommandIcon,
  DashboardSquare01Icon,
  Database01Icon,
  File01Icon,
  HelpCircleIcon,
  SearchIcon,
  Settings05Icon,
  Shield01Icon,
  SlidersHorizontalIcon,
  ThreeDViewIcon,
} from '@hugeicons/core-free-icons';

const data = {
  user: {
    name: 'Joey',
    email: 'local · can0',
    avatar: '',
  },
  navMain: [
    {
      title: 'Overview',
      url: '#',
      icon: <HugeiconsIcon icon={DashboardSquare01Icon} strokeWidth={2} />,
    },
    {
      title: 'Visualizer',
      url: '#',
      icon: <HugeiconsIcon icon={ThreeDViewIcon} strokeWidth={2} />,
    },
    {
      title: 'Joints',
      url: '#',
      icon: <HugeiconsIcon icon={SlidersHorizontalIcon} strokeWidth={2} />,
    },
    {
      title: 'Safety',
      url: '#',
      icon: <HugeiconsIcon icon={Shield01Icon} strokeWidth={2} />,
    },
    {
      title: 'Telemetry',
      url: '#',
      icon: <HugeiconsIcon icon={ChartHistogramIcon} strokeWidth={2} />,
    },
  ],
  navSecondary: [
    {
      title: 'Settings',
      url: '#',
      icon: <HugeiconsIcon icon={Settings05Icon} strokeWidth={2} />,
    },
    {
      title: 'Docs',
      url: '#',
      icon: <HugeiconsIcon icon={HelpCircleIcon} strokeWidth={2} />,
    },
    {
      title: 'Search',
      url: '#',
      icon: <HugeiconsIcon icon={SearchIcon} strokeWidth={2} />,
    },
  ],
  documents: [
    {
      name: 'golden_pose',
      url: '#',
      icon: <HugeiconsIcon icon={Database01Icon} strokeWidth={2} />,
    },
    {
      name: 'bench_default',
      url: '#',
      icon: <HugeiconsIcon icon={Analytics01Icon} strokeWidth={2} />,
    },
    {
      name: 'tuning_sweep',
      url: '#',
      icon: <HugeiconsIcon icon={File01Icon} strokeWidth={2} />,
    },
  ],
};

export function AppSidebar({ ...props }: React.ComponentProps<typeof Sidebar>) {
  return (
    <Sidebar collapsible="offcanvas" {...props}>
      <SidebarHeader>
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
      </SidebarHeader>
      <SidebarContent>
        <NavMain items={data.navMain} />
        <NavDocuments items={data.documents} />
        <NavSecondary items={data.navSecondary} className="mt-auto" />
      </SidebarContent>
      <SidebarFooter>
        <NavUser user={data.user} />
      </SidebarFooter>
    </Sidebar>
  );
}
