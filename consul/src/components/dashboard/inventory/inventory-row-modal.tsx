import { useEffect, useMemo, useState } from 'react';

import { ActuatorDetailBody } from '@/components/dashboard/inventory/actuator-detail-body';
import {
  KIND_OPTIONS,
  PRESET_OPTIONS_WITH_UNASSIGNED,
  STATUS_OPTIONS,
  inventoryModalContentClassName,
} from '@/components/dashboard/inventory/constants';
import { isSubsystemInteractive } from '@/components/dashboard/inventory/subsystem-interactive';
import type { InventoryRow } from '@/components/dashboard/inventory/types';
import { INVENTORY_GROUP_LABELS, type InventoryGroup } from '@/data/robot-inventory';
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
import { Input } from '@/components/ui/input';
import { Label } from '@/components/ui/label';
import {
  Select,
  SelectContent,
  SelectGroup,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from '@/components/ui/select';
import { Separator } from '@/components/ui/separator';
import { useActiveReportingLease } from '@/hooks/use-active-reporting-lease';
import { useRobotStore } from '@/state/robotStore';
import { HugeiconsIcon } from '@hugeicons/react';
import {
  ArrowLeft01Icon,
  ArrowRight01Icon,
} from '@hugeicons/core-free-icons';

export type InventoryIdentityPatch = {
  name: string;
  group: InventoryGroup;
  kind: InventoryRow['kind'];
  status: InventoryRow['status'];
  preset: string;
  limit: string;
};

type InventoryRowModalProps = {
  item: InventoryRow;
  items: InventoryRow[];
  open: boolean;
  onOpenChange: (open: boolean) => void;
  onNavigate: (item: InventoryRow) => void;
  onApply?: (itemId: number, patch: InventoryIdentityPatch) => void;
};

const GROUP_OPTIONS = (
  Object.entries(INVENTORY_GROUP_LABELS) as Array<[InventoryGroup, string]>
).map(([value, label]) => ({ value, label }));

/** Centered single-subsystem detail modal — replaces the right drawer. */
export function InventoryRowModal({
  item,
  items,
  open,
  onOpenChange,
  onNavigate,
  onApply,
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
  const [nameDraft, setNameDraft] = useState(item.name);
  const [groupDraft, setGroupDraft] = useState<InventoryGroup>(item.group);
  const [kindDraft, setKindDraft] = useState(item.kind);
  const [statusDraft, setStatusDraft] = useState(item.status);
  const [presetDraft, setPresetDraft] = useState(item.preset);
  const [limitDraft, setLimitDraft] = useState(item.limit);
  const [moveToDraft, setMoveToDraft] = useState('');

  useEffect(() => {
    setNameDraft(item.name);
    setGroupDraft(item.group);
    setKindDraft(item.kind);
    setStatusDraft(item.status);
    setPresetDraft(item.preset);
    setLimitDraft(item.limit);
    setMoveToDraft('');
  }, [item.id, item.name, item.group, item.kind, item.status, item.preset, item.limit]);

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
        <DialogHeader className="pr-16">
          <div className="flex items-start justify-between gap-3">
            <div className="min-w-0 flex-1">
              <DialogTitle className="truncate font-mono text-base tracking-tight">
                {item.name}
              </DialogTitle>
              <DialogDescription className="mt-1 font-mono text-[11px] tracking-wide">
                {INVENTORY_GROUP_LABELS[item.group]} · {item.kind} · {item.node}
              </DialogDescription>
            </div>
            <div className="flex shrink-0 items-center gap-1.5">
              <Badge
                variant="outline"
                className="font-mono text-[10px] uppercase tracking-[0.14em]"
              >
                {item.status}
              </Badge>
              {showEnhancedLogging ? (
                <Badge
                  variant="secondary"
                  className="font-mono text-[10px] uppercase tracking-[0.14em]"
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
          <div className="mt-3 flex items-center gap-2">
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
        </DialogHeader>

        <DialogBody className="flex flex-col gap-5">
          <section className="flex flex-col gap-3" aria-label="Identity">
            <h3 className="font-mono text-[10px] uppercase tracking-[0.14em] text-muted-foreground">
              Identity
            </h3>
            <div className="grid gap-3 sm:grid-cols-2">
              <div className="flex flex-col gap-2 sm:col-span-2">
                <Label htmlFor="subsystem-name">Name</Label>
                <Input
                  id="subsystem-name"
                  value={nameDraft}
                  onChange={(event) => setNameDraft(event.target.value)}
                />
              </div>
              <div className="flex flex-col gap-2">
                <Label htmlFor="subsystem-group">Location</Label>
                <Select
                  value={groupDraft}
                  onValueChange={(value) =>
                    setGroupDraft(value as InventoryGroup)
                  }
                  items={GROUP_OPTIONS}
                >
                  <SelectTrigger id="subsystem-group" className="w-full">
                    <SelectValue placeholder="Select location" />
                  </SelectTrigger>
                  <SelectContent>
                    <SelectGroup>
                      {GROUP_OPTIONS.map((option) => (
                        <SelectItem key={option.value} value={option.value}>
                          {option.label}
                        </SelectItem>
                      ))}
                    </SelectGroup>
                  </SelectContent>
                </Select>
              </div>
              <div className="flex flex-col gap-2">
                <Label htmlFor="subsystem-kind">Kind</Label>
                <Select
                  value={kindDraft}
                  onValueChange={(value) =>
                    setKindDraft(value as InventoryRow['kind'])
                  }
                  items={[...KIND_OPTIONS]}
                  disabled={!interactive}
                >
                  <SelectTrigger id="subsystem-kind" className="w-full">
                    <SelectValue placeholder="Select kind" />
                  </SelectTrigger>
                  <SelectContent>
                    <SelectGroup>
                      {KIND_OPTIONS.map((option) => (
                        <SelectItem key={option.value} value={option.value}>
                          {option.label}
                        </SelectItem>
                      ))}
                    </SelectGroup>
                  </SelectContent>
                </Select>
              </div>
              <div className="flex flex-col gap-2">
                <Label htmlFor="subsystem-status">Status</Label>
                <Select
                  value={statusDraft}
                  onValueChange={(value) =>
                    setStatusDraft(value as InventoryRow['status'])
                  }
                  items={[...STATUS_OPTIONS]}
                  disabled={!interactive}
                >
                  <SelectTrigger id="subsystem-status" className="w-full">
                    <SelectValue placeholder="Select status" />
                  </SelectTrigger>
                  <SelectContent>
                    <SelectGroup>
                      {STATUS_OPTIONS.map((option) => (
                        <SelectItem key={option.value} value={option.value}>
                          {option.label}
                        </SelectItem>
                      ))}
                    </SelectGroup>
                  </SelectContent>
                </Select>
              </div>
              <div className="flex flex-col gap-2">
                <Label htmlFor="subsystem-preset">Preset</Label>
                <Select
                  value={presetDraft}
                  onValueChange={setPresetDraft}
                  items={[...PRESET_OPTIONS_WITH_UNASSIGNED]}
                >
                  <SelectTrigger id="subsystem-preset" className="w-full">
                    <SelectValue placeholder="Select preset" />
                  </SelectTrigger>
                  <SelectContent>
                    <SelectGroup>
                      {PRESET_OPTIONS_WITH_UNASSIGNED.map((option) => (
                        <SelectItem key={option.value} value={option.value}>
                          {option.label}
                        </SelectItem>
                      ))}
                    </SelectGroup>
                  </SelectContent>
                </Select>
              </div>
            </div>
            {!interactive ? (
              <p className="text-xs text-muted-foreground" role="status">
                Offline or unconfigured — name, location, and preset stay
                editable; telemetry and motion stay locked until online and
                configured.
              </p>
            ) : null}
          </section>

          {item.kind === 'actuator' ? (
            <>
              <Separator className="bg-line" />
              <ActuatorDetailBody
                item={item}
                interactive={interactive}
                limitDraft={limitDraft}
                onLimitDraftChange={setLimitDraft}
                onApplyRange={(range) => {
                  setLimitDraft(range);
                  onApply?.(item.id, {
                    name: nameDraft,
                    group: groupDraft,
                    kind: kindDraft,
                    status: statusDraft,
                    preset: presetDraft,
                    limit: range,
                  });
                }}
                moveToDraft={moveToDraft}
                onMoveToDraftChange={setMoveToDraft}
              />
            </>
          ) : (
            <>
              <Separator className="bg-line" />
              <section
                className={!interactive ? 'opacity-40' : undefined}
                aria-disabled={!interactive}
              >
                <h3 className="mb-3 font-mono text-[10px] uppercase tracking-[0.14em] text-muted-foreground">
                  Reading
                </h3>
                <div
                  className={
                    !interactive ? 'pointer-events-none select-none' : undefined
                  }
                  inert={!interactive ? true : undefined}
                >
                  <div className="grid gap-3 sm:grid-cols-2">
                    <div className="flex flex-col gap-2">
                      <Label htmlFor="subsystem-value">Value</Label>
                      <Input
                        id="subsystem-value"
                        className="font-mono"
                        defaultValue={item.value}
                        disabled={!interactive}
                      />
                    </div>
                    <div className="flex flex-col gap-2">
                      <Label htmlFor="subsystem-limit">Range</Label>
                      <Input
                        id="subsystem-limit"
                        className="font-mono"
                        value={limitDraft}
                        disabled={!interactive}
                        onChange={(event) => setLimitDraft(event.target.value)}
                      />
                    </div>
                  </div>
                </div>
              </section>
            </>
          )}
        </DialogBody>

        <DialogFooter>
          <DialogClose asChild>
            <Button type="button" variant="outline">
              Cancel
            </Button>
          </DialogClose>
          <Button
            type="button"
            onClick={() => {
              onApply?.(item.id, {
                name: nameDraft,
                group: groupDraft,
                kind: kindDraft,
                status: statusDraft,
                preset: presetDraft,
                limit: limitDraft,
              });
              onOpenChange(false);
            }}
          >
            Apply
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
