import { useState } from 'react';
import { toast } from 'sonner';

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
import { cn } from '@/lib/utils';
import { useInventoryOverridesStore } from '@/state/inventoryOverridesStore';

type PresetCellProps = {
  itemId: number;
  preset: string;
  jointName: string;
};

/** Catalog preset tags only — durable membership edits live on Hardware. */
export function PresetCell({ itemId, preset, jointName }: PresetCellProps) {
  const [editing, setEditing] = useState(false);
  const applyPatch = useInventoryOverridesStore((state) => state.applyPatch);
  const overridePreset = useInventoryOverridesStore(
    (state) => state.overrides[itemId]?.preset,
  );
  const assignedPreset = overridePreset ?? preset;
  const isUnassigned = assignedPreset === 'unassigned';

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
            isUnassigned ? 'Assign preset tag' : `Edit preset ${assignedPreset}`
          }
          onClick={() => setEditing(true)}
        >
          {isUnassigned ? 'Assign preset' : assignedPreset}
        </Button>
      ) : (
        <>
          <Label htmlFor={`${itemId}-preset`} className="sr-only">
            Preset tag for {jointName}
          </Label>
          <Select
            items={[...PRESET_OPTIONS]}
            defaultOpen
            value={isUnassigned ? undefined : assignedPreset}
            onValueChange={(value) => {
              applyPatch(itemId, { preset: value });
              setEditing(false);
              toast.message(`Preset tag ${value} saved locally (not durable config).`);
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
    </>
  );
}
