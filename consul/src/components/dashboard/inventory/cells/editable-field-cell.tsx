import { useState } from 'react';
import { toast } from 'sonner';

import { Button } from '@/components/ui/button';
import { Input } from '@/components/ui/input';
import { Label } from '@/components/ui/label';

type EditableFieldCellProps = {
  itemId: number;
  itemName: string;
  field: 'value' | 'limit';
  label: string;
  defaultValue: string;
  inputClassName?: string;
};

/** Read-only text until clicked — avoids 58 Inputs on first paint. */
export function EditableFieldCell({
  itemId,
  itemName,
  field,
  label,
  defaultValue,
  inputClassName = 'h-8 w-20',
}: EditableFieldCellProps) {
  const [editing, setEditing] = useState(false);

  if (!editing) {
    return (
      <Button
        type="button"
        variant="ghost"
        className={`justify-end px-1 font-mono text-xs text-foreground ${inputClassName}`}
        onClick={() => setEditing(true)}
      >
        {defaultValue}
      </Button>
    );
  }

  return (
    <form
      onSubmit={(event) => {
        event.preventDefault();
        setEditing(false);
        toast.promise(new Promise((resolve) => setTimeout(resolve, 1000)), {
          loading: `Saving ${itemName}`,
          success: 'Done',
          error: 'Error',
        });
      }}
    >
      <Label htmlFor={`${itemId}-${field}`} className="sr-only">
        {label}
      </Label>
      <Input
        autoFocus
        className={`${inputClassName} border-transparent bg-transparent text-right font-mono text-xs shadow-none hover:bg-input/30 focus-visible:border focus-visible:bg-background dark:bg-transparent dark:hover:bg-input/30 dark:focus-visible:bg-input/30`}
        defaultValue={defaultValue}
        id={`${itemId}-${field}`}
        onBlur={() => setEditing(false)}
      />
    </form>
  );
}
