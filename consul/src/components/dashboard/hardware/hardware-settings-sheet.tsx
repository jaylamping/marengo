import { Link } from 'react-router-dom';

import { CommissioningBadgeChip } from '@/components/dashboard/hardware/commissioning-badge';
import { SetLimitsPanel } from '@/components/dashboard/inventory/set-limits-panel';
import { Badge } from '@/components/ui/badge';
import {
  Sheet,
  SheetContent,
  SheetDescription,
  SheetHeader,
  SheetTitle,
} from '@/components/ui/sheet';
import { useActiveReportingLease } from '@/hooks/use-active-reporting-lease';
import { cn } from '@/lib/utils';
import { useRobotStore } from '@/state/robotStore';

import type { HardwareJointRow } from '@/components/dashboard/hardware/build-hardware-rows';

/** Avoid auto-focusing the Set Limits help trigger (Radix Tooltip opens on focus). */
function focusSheetOnOpen(event: Event) {
  event.preventDefault();
  (event.currentTarget as HTMLElement).focus();
}

function formatLimit(value: number | null): string {
  if (value === null || !Number.isFinite(value)) {
    return '—';
  }
  return value.toFixed(3);
}

type HardwareSettingsSheetProps = {
  row: HardwareJointRow | null;
  open: boolean;
  onOpenChange: (open: boolean) => void;
  onApplyRange: (range: string) => void;
};

export function HardwareSettingsSheet({
  row,
  open,
  onOpenChange,
  onApplyRange,
}: HardwareSettingsSheetProps) {
  const operationalMode = useRobotStore((s) => s.operationalMode);
  const leaseEnabled = open && row != null && row.onCan;
  const leaseState = useActiveReportingLease({
    joint: leaseEnabled ? row.joint : null,
    enabled: leaseEnabled,
  });
  const showEnhancedLogging =
    leaseState === 'requested' && operationalMode !== 'ACTIVE';

  return (
    <Sheet open={open} onOpenChange={onOpenChange}>
      <SheetContent
        side="right"
        variant="panel"
        className="w-full sm:max-w-md"
        showOverlay
        onOpenAutoFocus={focusSheetOnOpen}
      >
        {row ? (
          <>
            <SheetHeader className="border-b border-line">
              <SheetTitle className="font-sans text-base tracking-tight">
                {row.joint}
              </SheetTitle>
              <SheetDescription className="micro-label flex flex-wrap items-center gap-2">
                <span>
                  {row.onCan ? `on can · id ${row.canId}` : 'description only'}
                </span>
                <CommissioningBadgeChip badge={row.badge} />
                {showEnhancedLogging ? (
                  <Badge
                    variant="outline"
                    className="font-mono text-[10px] uppercase tracking-[0.14em] text-accent"
                    data-testid="hardware-enhanced-logging"
                  >
                    Enhanced logging
                  </Badge>
                ) : null}
              </SheetDescription>
            </SheetHeader>

            <div className="flex-1 overflow-y-auto px-6 py-4 flex flex-col gap-4">
              <section className="flex flex-col gap-2">
                <h3 className="font-mono text-[10px] uppercase tracking-[0.14em] text-muted-foreground">
                  Limits (ADR 0012)
                </h3>
                <LimitField
                  label="Live hard range (rad)"
                  value={row.liveRange}
                  tag="live Davout snapshot · Set Limits SoT"
                  highlight
                />
                <LimitField
                  label="Disk hard min (rad)"
                  value={formatLimit(row.diskHardLower)}
                  tag="write-behind · motors.yaml"
                />
                <LimitField
                  label="Disk hard max (rad)"
                  value={formatLimit(row.diskHardUpper)}
                  tag="write-behind · motors.yaml"
                />
                <LimitField
                  label="Disk soft min (rad)"
                  value={formatLimit(row.diskSoftLower)}
                  tag="write-behind · control.yaml"
                />
                <LimitField
                  label="Disk soft max (rad)"
                  value={formatLimit(row.diskSoftUpper)}
                  tag="write-behind · control.yaml"
                />
              </section>

              <section className="flex flex-col gap-2">
                <h3 className="font-mono text-[10px] uppercase tracking-[0.14em] text-muted-foreground">
                  Motors map
                </h3>
                <LimitField
                  label="Direction"
                  value={row.direction !== null ? String(row.direction) : '—'}
                  tag="motors.yaml"
                />
                <LimitField
                  label="Motor type"
                  value={row.motorType ?? '—'}
                  tag="motors.yaml"
                />
                <LimitField
                  label="CAN"
                  value={
                    row.onCan
                      ? `${row.canInterface ?? 'can'} · id ${row.canId}`
                      : '—'
                  }
                  tag="motors.yaml"
                />
              </section>

              {row.warnings.length > 0 ? (
                <section className="flex flex-col gap-2">
                  <h3 className="font-mono text-[10px] uppercase tracking-[0.14em] text-accent">
                    Completeness (warn only)
                  </h3>
                  {row.warnings.map((w) => (
                    <div
                      key={`${w.code}-${w.message}`}
                      className="rounded-sm border border-accent/30 bg-accent/5 px-3 py-2 text-xs"
                    >
                      <Badge
                        variant="outline"
                        className="mb-1 font-mono text-[10px] uppercase tracking-[0.14em] text-accent"
                      >
                        {w.code}
                      </Badge>
                      <p className="text-foreground">{w.message}</p>
                    </div>
                  ))}
                </section>
              ) : null}

              <section
                className="flex flex-col gap-2"
                data-testid="hardware-commissioning-commands"
              >
                <h3 className="font-mono text-[10px] uppercase tracking-[0.14em] text-muted-foreground">
                  Commissioning · Set Limits + Set Zero
                </h3>
                <p className="text-xs text-muted-foreground">
                  Apply Limits replaces the durable hard/soft SoT on the Pi
                  (live Davout + motors/control write-behind; URDF expand-only).
                  Deploy preserves those taught envelopes by default. Motors must
                  stay not ACTIVE while listening; Set Zero captures mechanical
                  reference (Ready follows wire Verified).
                </p>
                <SetLimitsPanel
                  jointName={row.joint}
                  currentLimit={row.liveRange}
                  onApplyRange={onApplyRange}
                />
              </section>
            </div>
          </>
        ) : null}
      </SheetContent>
    </Sheet>
  );
}

function LimitField({
  label,
  value,
  tag,
  highlight,
}: {
  label: string;
  value: string;
  tag: string;
  highlight?: boolean;
}) {
  return (
    <div
      className={cn(
        'grid grid-cols-[1fr_auto] items-start gap-x-3 border-b border-line py-2',
        highlight && 'border-l-2 border-l-accent pl-2',
      )}
    >
      <div className="min-w-0">
        <div className="text-sm text-foreground">{label}</div>
        <div className="micro-label mt-0.5">{tag}</div>
      </div>
      <div className="data-value text-sm tabular-nums text-foreground">{value}</div>
    </div>
  );
}

/** Read-only limits for Inventory — durable Set Limits moved to Hardware. */
export function InventoryLimitsReadOnly({
  jointName,
  liveRange,
}: {
  jointName: string;
  liveRange: string;
}) {
  return (
    <section
      className="flex flex-col gap-3 rounded-sm border border-line p-3 panel-brackets"
      data-testid="inventory-limits-readonly"
      aria-label="Joint limits (read-only)"
    >
      <div className="flex items-center justify-between gap-2">
        <h3 className="font-mono text-[10px] uppercase tracking-[0.14em] text-muted-foreground">
          Limits
        </h3>
        <Badge variant="outline" className="font-mono text-[10px] uppercase tracking-[0.14em]">
          read-only
        </Badge>
      </div>
      <div className="font-mono text-sm tabular-nums text-foreground">
        Range (live): {liveRange}
      </div>
      <p className="text-xs text-muted-foreground">
        Durable Set Limits and URDF expand live on Hardware only.
      </p>
      <Link
        to="/hardware"
        className="inline-flex h-6 items-center justify-center rounded-md border border-line bg-surface-2/50 px-2 text-xs font-medium text-foreground hover:border-line-strong hover:bg-surface-2"
      >
        Calibrate on Hardware
      </Link>
      <span className="sr-only">Joint {jointName}</span>
    </section>
  );
}
