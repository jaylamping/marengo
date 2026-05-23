import { useMatches } from 'react-router-dom';

import { SiteHeaderStatusBadges } from '@/components/dashboard/site-header/site-header-status-badges';
import { siteHeaderConfig } from '@/data/site-header';
import { getRouteHeader } from '@/lib/route-handle';
import { Separator } from '@/components/ui/separator';
import { SidebarTrigger } from '@/components/ui/sidebar';

export function SiteHeader() {
  const matches = useMatches();
  const routeHeader = getRouteHeader(matches);

  const title = routeHeader?.title ?? siteHeaderConfig.title;
  const subtitle = routeHeader?.subtitle ?? siteHeaderConfig.subtitle;

  return (
    <header className="flex h-(--header-height) shrink-0 items-center gap-2 border-b transition-[width,height] ease-linear group-has-data-[collapsible=icon]/sidebar-wrapper:h-(--header-height)">
      <div className="flex w-full items-center gap-1 px-4 lg:gap-2 lg:px-6">
        <SidebarTrigger className="-ml-1" />
        <Separator
          orientation="vertical"
          className="mx-2 h-4 data-vertical:self-auto"
        />
        <div className="flex min-w-0 flex-1 items-center gap-3">
          <h1 className="text-base font-medium">{title}</h1>
          {subtitle ? (
            <span className="hidden text-sm text-muted-foreground sm:inline">
              {subtitle}
            </span>
          ) : null}
        </div>
        <SiteHeaderStatusBadges />
      </div>
    </header>
  );
}
