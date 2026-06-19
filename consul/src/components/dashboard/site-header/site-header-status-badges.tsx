import { siteHeaderConfig } from '@/data/site-header';
import { Badge } from '@/components/ui/badge';
import {
  chappeConnectionErrDetail,
  chappeMisconfigHint,
  isChappeLive,
  resolveChappeEndpoints,
} from '@/lib/chappe-config';
import { useHostMetricsStore } from '@/state/hostMetricsStore';
import { useRobotStore } from '@/state/robotStore';

export function SiteHeaderStatusBadges() {
  const connected = useRobotStore((s) => s.connected);
  const operationalMode = useRobotStore((s) => s.operationalMode);
  const gatewayError = useRobotStore((s) => s.gatewayError);
  const transportMode = useHostMetricsStore((s) => s.transportMode);
  const resolution = resolveChappeEndpoints();
  const live = resolution.endpoints !== null;
  const chappeErrDetail =
    live && !connected && gatewayError
      ? chappeConnectionErrDetail(resolution, chappeMisconfigHint())
      : null;

  return (
    <div className="flex items-center gap-2">
      <Badge variant="outline" className="hidden font-mono text-xs sm:inline-flex">
        {siteHeaderConfig.bus}
      </Badge>
      {live ? (
        <>
          {transportMode !== 'offline' ? (
            <Badge variant="outline" className="font-mono text-xs">
              {transportMode === 'webtransport' ? 'WT' : 'HTTP'}
            </Badge>
          ) : null}
          <Badge
            variant={connected ? 'default' : 'destructive'}
            className="font-mono text-xs"
            title={chappeErrDetail ?? undefined}
          >
            {connected
              ? operationalMode ?? 'LIVE'
              : gatewayError
                ? 'CHAPPE ERR'
                : 'CONNECTING'}
          </Badge>
        </>
      ) : (
        <Badge variant="secondary" className="font-mono text-xs">
          WIREFRAME
        </Badge>
      )}
    </div>
  );
}
