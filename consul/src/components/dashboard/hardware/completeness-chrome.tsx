import { Alert02Icon } from '@hugeicons/core-free-icons';
import { HugeiconsIcon } from '@hugeicons/react';

import { cn } from '@/lib/utils';

export function CompletenessSummaryBadge({
  count,
  className,
}: {
  count: number;
  className?: string;
}) {
  if (count <= 0) {
    return (
      <span
        className={cn(
          'inline-flex h-8 items-center gap-1.5 rounded-lg border border-line bg-surface-1 px-2.5 text-muted-foreground',
          className,
        )}
        title="No completeness warnings"
      >
        <span className="micro-label">0 gaps</span>
      </span>
    );
  }
  return (
    <span
      className={cn(
        'inline-flex h-8 items-center gap-1.5 rounded-lg border border-line bg-surface-1 px-2.5 text-accent',
        className,
      )}
      title={`${count} completeness gaps — warn only, actions not blocked`}
    >
      <HugeiconsIcon icon={Alert02Icon} size={14} />
      <span className="data-value text-[11px]">{count}</span>
      <span className="micro-label text-accent/70">gaps</span>
    </span>
  );
}

export function JointStatusDot({
  onCan,
  hasWarnings,
  className,
}: {
  onCan: boolean;
  hasWarnings: boolean;
  className?: string;
}) {
  return (
    <span
      className={cn(
        'led',
        hasWarnings ? 'led-accent' : onCan ? 'led-ok' : '',
        className,
      )}
      aria-hidden
    />
  );
}

export function StatusLegend({
  onCanCount,
  gapCount,
  descriptionOnlyCount,
  className,
}: {
  onCanCount: number;
  gapCount: number;
  descriptionOnlyCount: number;
  className?: string;
}) {
  return (
    <div className={cn('flex flex-wrap items-center gap-4', className)}>
      <span className="micro-label flex items-center gap-1.5">
        <span className="led led-ok" />
        on can · {onCanCount}
      </span>
      <span className="micro-label flex items-center gap-1.5">
        <span className="led led-accent" />
        gap · {gapCount}
      </span>
      <span className="micro-label flex items-center gap-1.5">
        <span className="led" />
        description only · {descriptionOnlyCount}
      </span>
    </div>
  );
}
