import { CommissioningBadgeChip } from '@/components/dashboard/hardware/commissioning-badge';
import {
  MASTER_LIMBS,
  aggregateWorstBadge,
  limbBadgeForMembers,
  limbReady,
  resolveJointBadge,
  robotReady,
  type CommissioningBadge,
  type JointFacetSnapshot,
} from '@/lib/commissioning';
import { cn } from '@/lib/utils';

type Props = {
  facets: JointFacetSnapshot[];
  className?: string;
};

function robotDisplayBadge(facets: JointFacetSnapshot[]): CommissioningBadge {
  const built = facets.filter((f) => f.online || f.motorMapped);
  if (built.length === 0) {
    return 'Offline';
  }
  if (robotReady(facets)) {
    return 'Ready';
  }
  return aggregateWorstBadge(built.map(resolveJointBadge));
}

export function CommissioningAggregation({ facets, className }: Props) {
  const byName = new Map(facets.map((f) => [f.name, f] as const));
  const robotIsReady = robotReady(facets);
  const robotBadge = robotDisplayBadge(facets);

  return (
    <div
      className={cn(
        'flex flex-wrap items-center gap-3 rounded-sm border border-line bg-surface-1 px-3 py-2',
        className,
      )}
      data-testid="commissioning-aggregation"
    >
      <div className="flex items-center gap-2" data-testid="robot-ready-badge">
        <span className="micro-label">Robot</span>
        <CommissioningBadgeChip badge={robotBadge} testId="robot-badge-chip" />
        <span className="sr-only">
          Robot Ready {robotIsReady ? 'true' : 'false'}
        </span>
      </div>
      {Object.entries(MASTER_LIMBS).map(([limb, members]) => {
        const limbFacets = members.map(
          (name) =>
            byName.get(name) ?? {
              name,
              online: false,
              motorMapped: false,
              fault: 0,
              outOfLimits: false,
              driveActive: false,
              homingState: undefined,
            },
        );
        const ready = limbReady(limbFacets);
        const badge = limbBadgeForMembers(limbFacets);
        return (
          <div
            key={limb}
            className="flex items-center gap-2"
            data-testid={`limb-ready-${limb}`}
            data-limb-ready={ready ? 'true' : 'false'}
          >
            <span className="micro-label">{limb.replaceAll('_', ' ')}</span>
            <CommissioningBadgeChip badge={badge} testId={`limb-badge-${limb}`} />
          </div>
        );
      })}
    </div>
  );
}
