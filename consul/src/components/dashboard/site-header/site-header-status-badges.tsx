import { siteHeaderConfig } from '@/data/site-header';
import { Badge } from '@/components/ui/badge';
import { isChappeLive } from '@/lib/chappe-config';
import { useRobotStore } from '@/state/robotStore';

export function SiteHeaderStatusBadges() {
  const connected = useRobotStore((s) => s.connected);
  const operationalMode = useRobotStore((s) => s.operationalMode);
  const gatewayError = useRobotStore((s) => s.gatewayError);
  const live = isChappeLive();

  return (
    <div className="flex items-center gap-2">
      <Badge variant="outline" className="hidden font-mono text-xs sm:inline-flex">
        {siteHeaderConfig.bus}
      </Badge>
      {live ? (
        <Badge
          variant={connected ? 'default' : 'destructive'}
          className="font-mono text-xs"
        >
          {connected
            ? operationalMode ?? 'LIVE'
            : gatewayError
              ? 'CHAPPE ERR'
              : 'CONNECTING'}
        </Badge>
      ) : (
        <Badge variant="secondary" className="font-mono text-xs">
          WIREFRAME
        </Badge>
      )}
    </div>
  );
}
