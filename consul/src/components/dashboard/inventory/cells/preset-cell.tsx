import { useState } from 'react';
import { toast } from 'sonner';

import { OverwritePresetActuatorDialog } from '@/components/dashboard/inventory/overwrite-preset-dialog';
import { PRESET_OPTIONS } from '@/components/dashboard/inventory/constants';
import { Button } from '@/components/ui/button';
import { Label } from '@/components/ui/label';
import {
  Select,
  SelectContent,
  SelectGroup,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from '@/components/ui/select';
import { isMappedBringupPreset } from '@/lib/bringup-presets';
import { applyActuatorConfig } from '@/lib/config-api';
import { queryClient } from '@/lib/query-client';
import { queryKeys } from '@/lib/query-keys';
import { cn } from '@/lib/utils';
import { useInventoryOverridesStore } from '@/state/inventoryOverridesStore';
import { useNeedsRestartStore } from '@/state/needsRestartStore';

type PresetCellProps = {
  itemId: number;
  preset: string;
  jointName: string;
};

type PendingOverwrite = {
  presetId: string;
  expectedRevision: string;
  before: { position_lower_rad: number; position_upper_rad: number } | null;
  after: { position_lower_rad: number; position_upper_rad: number } | null;
};

/** Text by default — Radix Select only mounts when editing. */
export function PresetCell({ itemId, preset, jointName }: PresetCellProps) {
  const [editing, setEditing] = useState(false);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [pending, setPending] = useState<PendingOverwrite | null>(null);
  const applyPatch = useInventoryOverridesStore((state) => state.applyPatch);
  const overridePreset = useInventoryOverridesStore(
    (state) => state.overrides[itemId]?.preset,
  );
  // Membership-derived preset wins for mapped bench_*; localStorage only for catalog tags.
  const assignedPreset =
    isMappedBringupPreset(preset) || preset !== 'unassigned'
      ? preset
      : (overridePreset ?? preset);
  const isUnassigned = assignedPreset === 'unassigned';

  const assignMapped = async (presetId: string) => {
    setBusy(true);
    setError(null);
    try {
      const preview = await applyActuatorConfig({
        target_profile: presetId,
        operator_id: 'consul',
        op: 'preview',
        joint: jointName,
      });
      if (!preview) {
        setError('Gateway preview failed');
        return;
      }
      if (preview.decision === 'unmapped_preset') {
        setError('Preset is not a bringup profile');
        return;
      }
      if (preview.decision === 'unsupported_membership') {
        toast.error(preview.message);
        return;
      }
      if (preview.decision === 'noop') {
        toast.message(`${jointName} already matches ${presetId}`);
        return;
      }
      if (preview.decision === 'add') {
        const result = await applyActuatorConfig({
          target_profile: presetId,
          expected_revision: preview.revision ?? undefined,
          operator_id: 'consul',
          op: 'add_joint',
          joint: jointName,
        });
        if (!result?.ok) {
          toast.error(result?.message ?? 'Add joint failed');
          return;
        }
        if (result.restart_required) {
          useNeedsRestartStore.getState().markNeedsRestart({
            profile: presetId,
            joint: jointName,
            reason: 'structural',
            expected_revision: result.revision ?? '',
          });
          useNeedsRestartStore
            .getState()
            .openRestartDialog({ reason: 'structural' });
        } else if (!result.applied_live) {
          toast.success(`Saved to ${presetId} (disk only)`);
        }
        await queryClient.invalidateQueries({
          queryKey: queryKeys.configSnapshot,
        });
        return;
      }
      if (preview.decision === 'overwrite') {
        setPending({
          presetId,
          expectedRevision: preview.revision ?? '',
          before: preview.before
            ? {
                position_lower_rad: preview.before.position_lower_rad,
                position_upper_rad: preview.before.position_upper_rad,
              }
            : null,
          after: preview.after
            ? {
                position_lower_rad: preview.after.position_lower_rad,
                position_upper_rad: preview.after.position_upper_rad,
              }
            : null,
        });
      }
    } finally {
      setBusy(false);
      setEditing(false);
    }
  };

  const confirmOverwrite = async () => {
    if (!pending) {
      return;
    }
    setBusy(true);
    setError(null);
    try {
      const result = await applyActuatorConfig({
        target_profile: pending.presetId,
        expected_revision: pending.expectedRevision,
        operator_id: 'consul',
        op: 'upsert_limits',
        joint: jointName,
        position_lower_rad: pending.after?.position_lower_rad,
        position_upper_rad: pending.after?.position_upper_rad,
      });
      if (!result?.ok) {
        setError(result?.message ?? 'Overwrite failed — refresh and retry');
        return;
      }
      if (result.applied_live) {
        toast.success(`Live limits applied for ${jointName}`);
      } else {
        toast.success(`Saved limits to ${pending.presetId} (not live)`);
      }
      setPending(null);
      await queryClient.invalidateQueries({
        queryKey: queryKeys.configSnapshot,
      });
    } finally {
      setBusy(false);
    }
  };

  return (
    <>
      {!editing ? (
        <Button
          type="button"
          variant="ghost"
          size="sm"
          className={cn(
            'h-8 px-2 font-mono text-xs',
            isUnassigned ? 'text-muted-foreground' : 'text-foreground',
          )}
          aria-label={
            isUnassigned ? 'Assign preset' : `Edit preset ${assignedPreset}`
          }
          onClick={() => setEditing(true)}
        >
          {isUnassigned ? 'Assign preset' : assignedPreset}
        </Button>
      ) : (
        <>
          <Label htmlFor={`${itemId}-preset`} className="sr-only">
            Preset
          </Label>
          <Select
            items={[...PRESET_OPTIONS]}
            defaultOpen
            value={isUnassigned ? undefined : assignedPreset}
            onValueChange={(value) => {
              if (isMappedBringupPreset(value)) {
                void assignMapped(value);
                return;
              }
              applyPatch(itemId, { preset: value });
              setEditing(false);
            }}
            onOpenChange={(open) => {
              if (!open) {
                setEditing(false);
              }
            }}
          >
            <SelectTrigger
              className="w-38 **:data-[slot=select-value]:block **:data-[slot=select-value]:truncate"
              size="sm"
              id={`${itemId}-preset`}
            >
              <SelectValue placeholder="Assign preset" />
            </SelectTrigger>
            <SelectContent align="end">
              <SelectGroup>
                {PRESET_OPTIONS.map((option) => (
                  <SelectItem key={option.value} value={option.value}>
                    {option.label}
                  </SelectItem>
                ))}
              </SelectGroup>
            </SelectContent>
          </Select>
        </>
      )}
      <OverwritePresetActuatorDialog
        open={pending !== null}
        joint={jointName}
        presetId={pending?.presetId ?? ''}
        expectedRevision={pending?.expectedRevision ?? ''}
        before={pending?.before ?? null}
        after={pending?.after ?? null}
        busy={busy}
        error={error}
        onCancel={() => {
          setPending(null);
          setError(null);
        }}
        onConfirm={() => {
          void confirmOverwrite();
        }}
      />
    </>
  );
}
