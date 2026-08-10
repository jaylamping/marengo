import { HugeiconsIcon } from '@hugeicons/react';
import {
  Alert02Icon,
  CheckmarkCircle01Icon,
  Loading03Icon,
} from '@hugeicons/core-free-icons';

import {
  statusToneClasses,
  type StatusTone,
} from '@/components/dashboard/metrics/status-badge';
import { isHealthyStatus } from '@/components/dashboard/inventory/utils';
import { TelemetryReferenceFacetBadge } from '@/components/dashboard/telemetry/telemetry-reference-facet';
import { Badge } from '@/components/ui/badge';
import { cn } from '@/lib/utils';

type InventoryStatusCellProps = {
  status: string;
  /** When set, show wire-gated Reference facet (Unknown if homing_state absent). */
  jointName?: string;
  kind?: string;
};

function statusTone(status: string): StatusTone {
  if (isHealthyStatus(status)) {
    return 'healthy';
  }
  if (status === 'Fault') {
    return 'fault';
  }
  if (status === 'Tuning') {
    return 'warning';
  }
  return 'muted';
}

export function InventoryStatusCell({
  status,
  jointName,
  kind,
}: InventoryStatusCellProps) {
  const tone = statusTone(status);
  const showReference = kind === 'actuator' && Boolean(jointName);

  return (
    <div className="flex flex-col items-start gap-1">
      <Badge
        variant="outline"
        className={cn(
          'gap-1 px-1.5 font-mono text-[10px] tracking-[0.04em]',
          statusToneClasses[tone],
        )}
      >
        {tone === 'healthy' ? (
          <HugeiconsIcon
            icon={CheckmarkCircle01Icon}
            strokeWidth={2}
            className="fill-ok text-ok"
          />
        ) : null}
        {tone === 'warning' ? (
          <HugeiconsIcon
            icon={Loading03Icon}
            strokeWidth={2}
            className="text-warning"
          />
        ) : null}
        {tone === 'fault' ? (
          <HugeiconsIcon
            icon={Alert02Icon}
            strokeWidth={2}
            className="text-fault"
          />
        ) : null}
        {status}
      </Badge>
      {showReference && jointName ? (
        <TelemetryReferenceFacetBadge jointName={jointName} />
      ) : null}
    </div>
  );
}
