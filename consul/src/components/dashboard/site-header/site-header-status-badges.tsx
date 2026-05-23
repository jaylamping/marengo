import { siteHeaderConfig } from '@/data/site-header';
import { Badge } from '@/components/ui/badge';

export function SiteHeaderStatusBadges() {
  return (
    <div className="flex items-center gap-2">
      <Badge variant="outline" className="hidden font-mono text-xs sm:inline-flex">
        {siteHeaderConfig.bus}
      </Badge>
      <Badge variant="secondary" className="font-mono text-xs">
        WIREFRAME
      </Badge>
    </div>
  );
}
