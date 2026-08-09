/**
 * PROTOTYPE — chrome shared by all three Hardware variants, so the variants
 * differ only in structure and not in incidental styling.
 */
import { Alert02Icon, Upload01Icon } from '@hugeicons/core-free-icons';
import { HugeiconsIcon } from '@hugeicons/react';

import { Button } from '@/components/ui/button';
import { cn } from '@/lib/utils';

import { PROTO_JOINTS, WARN_COUNT, type ProtoJoint } from './mock-hardware';

export function PageTitle({ note }: { note: string }) {
  return (
    <div>
      <h1 className="font-sans text-lg tracking-tight text-foreground">Hardware</h1>
      <p className="micro-label">{note}</p>
    </div>
  );
}

/** Quiet completeness readout — warns, never blocks. */
export function CompletenessBadge({ className }: { className?: string }) {
  return (
    <span
      className={cn(
        'inline-flex h-8 items-center gap-1.5 rounded-lg border border-line bg-surface-1 px-2.5 text-accent',
        className,
      )}
      title={`${WARN_COUNT} completeness gaps — warn only`}
    >
      <HugeiconsIcon icon={Alert02Icon} size={14} />
      <span className="data-value text-[11px]">{WARN_COUNT}</span>
      <span className="micro-label text-accent/70">gaps</span>
    </span>
  );
}

export function ImportButton({ onImport }: { onImport: () => void }) {
  return (
    <Button type="button" size="sm" variant="secondary" className="gap-1.5" onClick={onImport}>
      <HugeiconsIcon icon={Upload01Icon} size={16} />
      Import
    </Button>
  );
}

export function JointStatusDot({ joint }: { joint: ProtoJoint }) {
  return (
    <span
      className={cn(
        'led',
        joint.completenessWarn ? 'led-accent' : joint.onCan ? 'led-ok' : '',
      )}
    />
  );
}

export function StatusLegend({ className }: { className?: string }) {
  const onCan = PROTO_JOINTS.filter((j) => j.onCan && !j.completenessWarn).length;
  const descriptionOnly = PROTO_JOINTS.filter((j) => !j.onCan && !j.completenessWarn).length;

  return (
    <div className={cn('flex items-center gap-4', className)}>
      <span className="micro-label flex items-center gap-1.5">
        <span className="led led-ok" />
        on can · {onCan}
      </span>
      <span className="micro-label flex items-center gap-1.5">
        <span className="led led-accent" />
        gap · {WARN_COUNT}
      </span>
      <span className="micro-label flex items-center gap-1.5">
        <span className="led" />
        description only · {descriptionOnly}
      </span>
    </div>
  );
}

export const openImportStub = () =>
  window.alert('PROTOTYPE — Import would open the resolve wizard (field-by-field merge).');
