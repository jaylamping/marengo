import { useState } from 'react';

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
};

/** Text by default — Radix Select only mounts when editing (was 22× on first paint). */
export function PresetCell({ itemId, preset }: PresetCellProps) {
  const [editing, setEditing] = useState(false);
  const applyPatch = useInventoryOverridesStore((state) => state.applyPatch);
  const assignedPreset = useInventoryOverridesStore(
    (state) => state.overrides[itemId]?.preset ?? preset,
  );
  const isUnassigned = assignedPreset === 'unassigned';

  if (!editing) {
    return (
      <Button
        type="button"
        variant="ghost"
        size="sm"
        className={cn(
          'h-8 px-2 font-mono text-xs',
          isUnassigned ? 'text-muted-foreground' : 'text-foreground',
        )}
        aria-label={isUnassigned ? 'Assign preset' : `Edit preset ${assignedPreset}`}
        onClick={() => setEditing(true)}
      >
        {isUnassigned ? 'Assign preset' : assignedPreset}
      </Button>
    );
  }

  return (
    <>
      <Label htmlFor={`${itemId}-preset`} className="sr-only">
        Preset
      </Label>
      <Select
        items={[...PRESET_OPTIONS]}
        defaultOpen
        value={isUnassigned ? undefined : assignedPreset}
        onValueChange={(value) => {
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
  );
}
