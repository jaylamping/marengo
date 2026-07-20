import { useEffect, useMemo, useRef, useState } from 'react';

import { ActuatorDetailBody } from '@/components/dashboard/inventory/actuator-detail-body';
import {
  PRESET_OPTIONS_WITH_UNASSIGNED,
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
import { useActiveReportingLease } from '@/hooks/use-active-reporting-lease';
import { cn } from '@/lib/utils';
import { useRobotStore } from '@/state/robotStore';
import { HugeiconsIcon } from '@hugeicons/react';
import {
  ArrowLeft01Icon,
  ArrowRight01Icon,
  Edit01Icon,
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

const headerChipTriggerClassName = cn(
  'h-7 w-auto min-w-0 max-w-[12rem] gap-1 border-line bg-transparent px-2',
  'font-mono text-[11px] tracking-wide text-muted-foreground',
  'hover:border-line-strong hover:bg-surface-2 hover:text-foreground',
  'data-[state=open]:border-line-strong data-[state=open]:bg-surface-2 data-[state=open]:text-foreground',
);

const headerEditButtonClassName = cn(
  'size-6 shrink-0 text-muted-foreground',
  'hover:bg-surface-2 hover:text-foreground',
);

/** Centered single-subsystem detail — chart-first; identity lives in the header. */
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
  const [presetDraft, setPresetDraft] = useState(item.preset);
  const [limitDraft, setLimitDraft] = useState(item.limit);
  const [moveToDraft, setMoveToDraft] = useState('');
  const [editingName, setEditingName] = useState(false);
  const [groupSelectOpen, setGroupSelectOpen] = useState(false);
  const [presetSelectOpen, setPresetSelectOpen] = useState(false);
  const nameInputRef = useRef<HTMLInputElement>(null);

  useEffect(() => {
    setNameDraft(item.name);
    setGroupDraft(item.group);
    setPresetDraft(item.preset);
    setLimitDraft(item.limit);
    setMoveToDraft('');
    setEditingName(false);
    setGroupSelectOpen(false);
    setPresetSelectOpen(false);
  }, [item.id, item.name, item.group, item.preset, item.limit]);

  useEffect(() => {
    if (!open) {
      setEditingName(false);
      setGroupSelectOpen(false);
      setPresetSelectOpen(false);
    }
  }, [open]);

  useEffect(() => {
    if (editingName) {
      nameInputRef.current?.focus();
      nameInputRef.current?.select();
    }
  }, [editingName]);

  const index = useMemo(
    () => items.findIndex((row) => row.id === item.id),
    [items, item.id],
  );
  const hasPrev = index > 0;
  const hasNext = index >= 0 && index < items.length - 1;
  const positionLabel =
    index >= 0 ? `${index + 1} / ${items.length}` : `— / ${items.length}`;

  const identityPatch = (): InventoryIdentityPatch => ({
    name: nameDraft,
    group: groupDraft,
    kind: item.kind,
    status: item.status,
    preset: presetDraft,
    limit: limitDraft,
  });

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
              <DialogTitle className="sr-only">{nameDraft || item.name}</DialogTitle>
              <div className="flex min-w-0 flex-wrap items-center gap-2">
                {editingName ? (
                  <Input
                    ref={nameInputRef}
                    id="subsystem-name"
                    aria-label="Name"
                    value={nameDraft}
                    onChange={(event) => setNameDraft(event.target.value)}
                    onBlur={() => setEditingName(false)}
                    onKeyDown={(event) => {
                      if (event.key === 'Enter' || event.key === 'Escape') {
                        event.preventDefault();
                        setEditingName(false);
                      }
                    }}
                    className={cn(
                      'h-auto min-w-0 flex-1 border-line bg-surface-2 px-2 py-1',
                      'font-mono text-base tracking-tight text-foreground',
                    )}
                  />
                ) : (
                  <div className="flex min-w-0 items-center gap-1">
                    <p className="truncate font-mono text-base tracking-tight text-foreground">
                      {nameDraft}
                    </p>
                    <Button
                      type="button"
                      variant="ghost"
                      size="icon-sm"
                      aria-label="Edit name"
                      className={headerEditButtonClassName}
                      onClick={() => setEditingName(true)}
                    >
                      <HugeiconsIcon icon={Edit01Icon} strokeWidth={2} className="size-3.5" />
                    </Button>
                  </div>
                )}
                <Badge
                  variant="outline"
                  className="shrink-0 font-mono text-[10px] uppercase tracking-[0.14em]"
                >
                  {item.status}
                </Badge>
              </div>
              <DialogDescription className="sr-only">
                {INVENTORY_GROUP_LABELS[groupDraft]} · {item.kind} · {item.node} ·{' '}
                {presetDraft}
              </DialogDescription>
            </div>
            <div className="flex shrink-0 items-center gap-1.5">
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
              {groupSelectOpen ? (
                <Select
                  open
                  onOpenChange={(next) => {
                    if (!next) {
                      setGroupSelectOpen(false);
                    }
                  }}
                  value={groupDraft}
                  onValueChange={(value) => {
                    const group = value as InventoryGroup;
                    setGroupDraft(group);
                    onApply?.(item.id, { ...identityPatch(), group });
                    setGroupSelectOpen(false);
                  }}
                  items={GROUP_OPTIONS}
                >
                  <SelectTrigger
                    id="subsystem-group"
                    aria-label="Location"
                    className={headerChipTriggerClassName}
                  >
                    <SelectValue placeholder="Location" />
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
              ) : (
                <span className="inline-flex min-w-0 items-center gap-0.5">
                  <span className="font-mono text-[11px] tracking-wide text-muted-foreground">
                    {INVENTORY_GROUP_LABELS[groupDraft]}
                  </span>
                  <Button
                    type="button"
                    variant="ghost"
                    size="icon-sm"
                    aria-label="Edit location"
                    className={headerEditButtonClassName}
                    onClick={() => setGroupSelectOpen(true)}
                  >
                    <HugeiconsIcon icon={Edit01Icon} strokeWidth={2} className="size-3" />
                  </Button>
                </span>
              )}
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
              <span className="font-mono text-[11px] text-muted-foreground" aria-hidden>
                ·
              </span>
              {presetSelectOpen ? (
                <Select
                  open
                  onOpenChange={(next) => {
                    if (!next) {
                      setPresetSelectOpen(false);
                    }
                  }}
                  value={presetDraft}
                  onValueChange={(value) => {
                    setPresetDraft(value);
                    onApply?.(item.id, { ...identityPatch(), preset: value });
                    setPresetSelectOpen(false);
                  }}
                  items={[...PRESET_OPTIONS_WITH_UNASSIGNED]}
                >
                  <SelectTrigger
                    id="subsystem-preset"
                    aria-label="Preset"
                    className={headerChipTriggerClassName}
                  >
                    <SelectValue placeholder="Preset" />
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
              ) : (
                <span className="inline-flex min-w-0 items-center gap-0.5">
                  <span className="font-mono text-[11px] tracking-wide text-muted-foreground">
                    {presetDraft}
                  </span>
                  <Button
                    type="button"
                    variant="ghost"
                    size="icon-sm"
                    aria-label="Edit preset"
                    className={headerEditButtonClassName}
                    onClick={() => setPresetSelectOpen(true)}
                  >
                    <HugeiconsIcon icon={Edit01Icon} strokeWidth={2} className="size-3" />
                  </Button>
                </span>
              )}
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
          {!interactive ? (
            <p className="mt-2 text-xs text-muted-foreground" role="status">
              Offline or unconfigured — name, location, and preset stay
              editable via the pencil controls; telemetry and motion stay locked
              until online and configured.
            </p>
          ) : null}
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
                onApply?.(item.id, { ...identityPatch(), limit: range });
              }}
              moveToDraft={moveToDraft}
              onMoveToDraftChange={setMoveToDraft}
            />
          ) : (
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
              onApply?.(item.id, identityPatch());
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
