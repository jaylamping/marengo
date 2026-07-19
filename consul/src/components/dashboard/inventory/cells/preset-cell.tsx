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

type PresetCellProps = {
  itemId: number;
  preset: string;
};

/** Text by default — Radix Select only mounts when assigning (was 22× on first paint). */
export function PresetCell({ itemId, preset }: PresetCellProps) {
  const [editing, setEditing] = useState(false);

  if (preset !== 'unassigned') {
    return <span className="font-mono text-xs">{preset}</span>;
  }

  if (!editing) {
    return (
      <Button
        type="button"
        variant="ghost"
        size="sm"
        className="h-8 px-2 font-mono text-xs text-muted-foreground"
        onClick={() => setEditing(true)}
      >
        Assign preset
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
