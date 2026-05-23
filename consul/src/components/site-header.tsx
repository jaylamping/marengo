import { Badge } from '@/components/ui/badge';
import { Separator } from '@/components/ui/separator';
import { SidebarTrigger } from '@/components/ui/sidebar';

export function SiteHeader() {
  return (
    <header className="flex h-(--header-height) shrink-0 items-center gap-2 border-b transition-[width,height] ease-linear group-has-data-[collapsible=icon]/sidebar-wrapper:h-(--header-height)">
      <div className="flex w-full items-center gap-1 px-4 lg:gap-2 lg:px-6">
        <SidebarTrigger className="-ml-1" />
        <Separator
          orientation="vertical"
          className="mx-2 h-4 data-vertical:self-auto"
        />
        <div className="flex min-w-0 flex-1 items-center gap-3">
          <h1 className="text-base font-medium">Overview</h1>
          <span className="hidden text-sm text-muted-foreground sm:inline">
            marengo_arm_4dof · bench
          </span>
        </div>
        <div className="flex items-center gap-2">
          <Badge variant="outline" className="hidden font-mono text-xs sm:inline-flex">
            vcan0
          </Badge>
          <Badge className="bg-green-600/90 font-mono text-xs hover:bg-green-600/90">
            LIVE
          </Badge>
        </div>
      </div>
    </header>
  );
}
