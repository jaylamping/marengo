import { cn } from '@/lib/utils';
import {
  selectNeedsRestart,
  useNeedsRestartStore,
} from '@/state/needsRestartStore';

type NeedsRestartBadgeProps = {
  /** Compact label for table Range cells. */
  variant?: 'needs' | 'pending';
  className?: string;
  /** When set, only render if this joint is pending (Range cell). */
  jointName?: string;
};

/**
 * Clickable chip that opens the shared restart confirmation dialog.
 * Header / actuator modal use variant "needs"; inventory Range uses "pending".
 */
export function NeedsRestartBadge({
  variant = 'needs',
  className,
  jointName,
}: NeedsRestartBadgeProps) {
  const needsRestart = useNeedsRestartStore(selectNeedsRestart);
  const isJointPending = useNeedsRestartStore((s) =>
    jointName ? s.isJointPending(jointName) : false,
  );
  const openRestartDialog = useNeedsRestartStore((s) => s.openRestartDialog);

  const visible = jointName ? isJointPending : needsRestart;
  if (!visible) {
    return null;
  }

  const label = variant === 'pending' ? 'Pending restart' : 'Needs restart';

  return (
    <button
      type="button"
      data-testid={
        variant === 'pending' ? 'pending-restart-badge' : 'needs-restart-badge'
      }
      className={cn(
        'inline-flex shrink-0 items-center border border-accent/50 bg-accent/10 px-2 py-0.5 font-mono text-[10px] uppercase tracking-[0.14em] text-accent transition-colors hover:bg-accent/20 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring',
        className,
      )}
      title="Hard limits saved — restart marengo-pi to load them into Davout"
      onClick={() => {
        openRestartDialog();
      }}
    >
      {label}
    </button>
  );
}
