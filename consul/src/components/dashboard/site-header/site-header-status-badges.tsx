import { siteHeaderConfig } from '@/data/site-header';
import { Badge } from '@/components/ui/badge';

export function SiteHeaderStatusBadges() {
  return (
    <div className="flex items-center gap-2">
      <Badge variant="outline" className="hidden font-mono text-xs sm:inline-flex">
        {siteHeaderConfig.bus}
      </Badge>
      <Badge className="bg-green-600/90 font-mono text-xs hover:bg-green-600/90">
        LIVE
      </Badge>
    </div>
  );
}
