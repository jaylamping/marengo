import { useEffect, useMemo, useState } from 'react';

import { ActuatorDetailBody } from '@/components/dashboard/inventory/actuator-detail-body';
import { inventoryModalContentClassName } from '@/components/dashboard/inventory/constants';
import { isSubsystemInteractive } from '@/components/dashboard/inventory/subsystem-interactive';
import type {
  InventoryIdentityPatch,
  InventoryRow,
} from '@/components/dashboard/inventory/types';
import { INVENTORY_GROUP_LABELS } from '@/data/robot-inventory';
import { Badge } from '@/components/ui/badge';
import { Button } from '@/components/ui/button';
import {
  Dialog,
  DialogBody,
  DialogClose,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from '@/components/ui/dialog';
import { NeedsRestartBadge } from '@/components/dashboard/needs-restart/needs-restart-badge';
import { useActiveReportingLease } from '@/hooks/use-active-reporting-lease';
import { useRobotStore } from '@/state/robotStore';
import { HugeiconsIcon } from '@hugeicons/react';
import {
  ArrowLeft01Icon,
  ArrowRight01Icon,
} from '@hugeicons/core-free-icons';

export type { InventoryIdentityPatch };

type InventoryRowModalProps = {
  item: InventoryRow;
  items: InventoryRow[];
  open: boolean;
  onOpenChange: (open: boolean) => void;
  onNavigate: (item: InventoryRow) => void;
  /** @deprecated Telemetry is read-only; identity Apply is no longer offered. */
  onApply?: (itemId: number, patch: Partial<InventoryIdentityPatch>) => void;
};

/**
 * Telemetry detail modal — live readings + read-only limits.
 * No preset assign, no durable Set Limits, no identity Apply.
 */
export function InventoryRowModal({
  item,
  items,
  open,
  onOpenChange,
  onNavigate,
}: InventoryRowModalProps) {
  const interactive = isSubsystemInteractive(item);
  const operationalMode = useRobotStore((s) => s.operationalMode);
  const leaseEnabled = open && interactive && item.kind === 'actuator';
  const leaseState = useActiveReportingLease({
    joint: leaseEnabled ? item.name : null,
    enabled: leaseEnabled,
  });
  const showEnhancedLogging =
    leaseState === 'requested' && operationalMode !== 'ACTIVE';
  const [limitDraft, setLimitDraft] = useState(item.limit);
  const [moveToDraft, setMoveToDraft] = useState('');

  useEffect(() => {
    setLimitDraft(item.limit);
    setMoveToDraft('');
  }, [item.id, item.limit]);

  const index = useMemo(
    () => items.findIndex((row) => row.id === item.id),
    [items, item.id],
  );
  const hasPrev = index > 0;
  const hasNext = index >= 0 && index < items.length - 1;
  const positionLabel =
    index >= 0 ? `${index + 1} / ${items.length}` : `— / ${items.length}`;

  useEffect(() => {
    if (!open) {
      return;
    }
    const onKeyDown = (event: KeyboardEvent) => {
      if (event.target instanceof HTMLElement) {
        const tag = event.target.tagName;
        if (tag === 'INPUT' || tag === 'TEXTAREA' || tag === 'SELECT') {
          return;
        }
        if (event.target.isContentEditable) {
          return;
        }
      }
      if (event.key === 'ArrowLeft' && hasPrev) {
        event.preventDefault();
        onNavigate(items[index - 1]!);
      } else if (event.key === 'ArrowRight' && hasNext) {
        event.preventDefault();
        onNavigate(items[index + 1]!);
      }
    };
    window.addEventListener('keydown', onKeyDown);
    return () => window.removeEventListener('keydown', onKeyDown);
  }, [open, hasPrev, hasNext, items, index, onNavigate]);

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent
        variant="panel"
        showCloseButton
        className={inventoryModalContentClassName}
        data-testid="inventory-row-modal"
      >
        <DialogHeader className="pr-3">
          <div className="flex items-start justify-between gap-3 pr-9">
            <div className="min-w-0 flex-1">
              <DialogTitle className="sr-only">{item.name}</DialogTitle>
              <div className="flex min-w-0 flex-wrap items-center gap-2">
                <p className="truncate font-mono text-base tracking-tight text-foreground">
                  {item.name}
                </p>
                <Badge
                  variant="outline"
                  className="shrink-0 font-mono text-[10px] uppercase tracking-[0.14em]"
                >
                  {item.status}
                </Badge>
              </div>
              <DialogDescription className="sr-only">
                {INVENTORY_GROUP_LABELS[item.group]} · {item.kind} · {item.node}
              </DialogDescription>
            </div>
            <div className="flex shrink-0 items-center gap-1.5">
              <NeedsRestartBadge variant="needs" />
              {showEnhancedLogging ? (
                <Badge
                  variant="outline"
                  className="border-ok bg-ok/10 font-mono text-[10px] uppercase tracking-[0.14em] text-ok"
                >
                  Enhanced logging
                </Badge>
              ) : null}
              {leaseState === 'failed' && operationalMode !== 'ACTIVE' ? (
                <Badge
                  variant="destructive"
                  className="font-mono text-[10px] uppercase tracking-[0.14em]"
                >
                  Logging failed
                </Badge>
              ) : null}
            </div>
          </div>
          <div className="mt-1.5 flex items-center gap-3">
            <div
              className="flex min-w-0 flex-1 flex-wrap items-center gap-x-1 gap-y-1.5"
              aria-label="Identity"
            >
              <span className="font-mono text-[11px] tracking-wide text-muted-foreground">
                {INVENTORY_GROUP_LABELS[item.group]}
              </span>
              <span className="font-mono text-[11px] text-muted-foreground" aria-hidden>
                ·
              </span>
              <span className="font-mono text-[11px] tracking-wide text-muted-foreground">
                {item.kind}
              </span>
              <span className="font-mono text-[11px] text-muted-foreground" aria-hidden>
                ·
              </span>
              <span className="truncate font-mono text-[11px] tracking-wide text-muted-foreground">
                {item.node}
              </span>
            </div>
            <div className="ml-auto flex shrink-0 items-center gap-1.5">
              <Button
                type="button"
                variant="panel"
                size="icon-sm"
                disabled={!hasPrev}
                aria-label="Previous subsystem"
                onClick={() => {
                  if (hasPrev) {
                    onNavigate(items[index - 1]!);
                  }
                }}
              >
                <HugeiconsIcon icon={ArrowLeft01Icon} strokeWidth={2} />
              </Button>
              <span className="font-mono text-[10px] uppercase tracking-[0.14em] text-muted-foreground">
                {positionLabel}
              </span>
              <Button
                type="button"
                variant="panel"
                size="icon-sm"
                disabled={!hasNext}
                aria-label="Next subsystem"
                onClick={() => {
                  if (hasNext) {
                    onNavigate(items[index + 1]!);
                  }
                }}
              >
                <HugeiconsIcon icon={ArrowRight01Icon} strokeWidth={2} />
              </Button>
            </div>
          </div>
          <p className="mt-2 text-xs text-muted-foreground" role="status">
            Telemetry is read-only. Set Limits, Set Zero, and Enable live on
            Hardware.
          </p>
        </DialogHeader>

        <DialogBody className="flex flex-col gap-5">
          {item.kind === 'actuator' ? (
            <ActuatorDetailBody
              item={item}
              interactive={interactive}
              limitDraft={limitDraft}
              onLimitDraftChange={setLimitDraft}
              onApplyRange={(range) => {
                setLimitDraft(range);
              }}
              moveToDraft={moveToDraft}
              onMoveToDraftChange={setMoveToDraft}
            />
          ) : (
            <section aria-label="Reading">
              <h3 className="mb-3 font-mono text-[10px] uppercase tracking-[0.14em] text-muted-foreground">
                Reading
              </h3>
              <div className="grid gap-3 sm:grid-cols-2">
                <ReadOnlyField label="Value" value={item.value} />
                <ReadOnlyField label="Range" value={item.limit} />
              </div>
            </section>
          )}
        </DialogBody>

        <DialogFooter>
          <DialogClose asChild>
            <Button type="button" variant="outline">
              Close
            </Button>
          </DialogClose>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}

function ReadOnlyField({ label, value }: { label: string; value: string }) {
  return (
    <div className="flex flex-col gap-1">
      <div className="font-mono text-[10px] uppercase tracking-[0.14em] text-muted-foreground">
        {label}
      </div>
      <div className="font-mono text-sm tabular-nums text-foreground">{value}</div>
    </div>
  );
}
