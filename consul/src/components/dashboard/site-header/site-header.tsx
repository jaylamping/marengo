import { useMatches } from 'react-router-dom';

import { siteHeaderPanelClassName } from '@/components/dashboard/sidebar/constants';
import { SiteHeaderStatusBadges } from '@/components/dashboard/site-header/site-header-status-badges';
import { siteHeaderConfig } from '@/data/site-header';
import { getRouteHeader } from '@/lib/route-handle';
import { cn } from '@/lib/utils';
import { Separator } from '@/components/ui/separator';
import { SidebarTrigger } from '@/components/ui/sidebar';

export function SiteHeader() {
  const matches = useMatches();
  const routeHeader = getRouteHeader(matches);

  const title = routeHeader?.title ?? siteHeaderConfig.title;
  const subtitle = routeHeader?.subtitle ?? siteHeaderConfig.subtitle;

  return (
    <header
      className={cn(
        'flex h-(--header-height) shrink-0 items-center gap-2 transition-[width,height] ease-linear group-has-data-[collapsible=icon]/sidebar-wrapper:h-(--header-height)',
        siteHeaderPanelClassName,
      )}
    >
      <div className="flex w-full items-center gap-1 px-4 lg:gap-2 lg:px-6">
        <SidebarTrigger className="-ml-1" />
        <Separator
          orientation="vertical"
          className="mx-2 h-4 data-vertical:self-auto"
        />
        <div className="flex min-w-0 flex-1 flex-col justify-center gap-0.5">
          <h1 className="micro-label leading-none text-foreground">{title}</h1>
          {subtitle ? (
            <span className="hidden font-mono text-[10px] leading-none tracking-[0.08em] text-faint sm:inline">
              {subtitle}
            </span>
          ) : null}
        </div>
        <SiteHeaderStatusBadges />
      </div>
    </header>
  );
}
