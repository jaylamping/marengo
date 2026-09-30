import {
  resolveTelemetryReferenceFacet,
  telemetryReferenceFacetLabel,
  type TelemetryReferenceFacet,
} from '@/lib/telemetry-facets';
import { Badge } from '@/components/ui/badge';
import { cn } from '@/lib/utils';
import { useRobotStore } from '@/state/robotStore';

const toneClass: Record<TelemetryReferenceFacet, string> = {
  unknown: 'border-border text-muted-foreground',
  ready: 'border-[color:var(--ok)]/40 text-[color:var(--ok)]',
  not_ready: 'border-warning/50 text-warning',
};

type TelemetryReferenceFacetBadgeProps = {
  jointName: string;
  className?: string;
};

/** Reference facet badge — Unknown when wire `homing_state` is absent. */
export function TelemetryReferenceFacetBadge({
  jointName,
  className,
}: TelemetryReferenceFacetBadgeProps) {
  const joint = useRobotStore((s) =>
    s.robotState?.joints.find((entry) => entry.name === jointName),
  );
  const facet = resolveTelemetryReferenceFacet(
    joint as { name?: string; homingState?: unknown } | undefined,
  );
  const label = telemetryReferenceFacetLabel(facet);

  return (
    <Badge
      variant="outline"
      data-testid="telemetry-reference-facet"
      data-facet={facet}
      title="Reference facet (wire-gated)"
      className={cn(
        'px-1.5 font-mono text-[10px] uppercase tracking-[0.04em]',
        toneClass[facet],
        className,
      )}
    >
      {label}
    </Badge>
  );
}
