import { useState } from 'react';

import { Button } from '@/components/ui/button';
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuSeparator,
  DropdownMenuTrigger,
} from '@/components/ui/dropdown-menu';
import { HugeiconsIcon } from '@hugeicons/react';
import { MoreVerticalCircle01Icon } from '@hugeicons/core-free-icons';

/** Menu content mounts only while open — 29× always-on menus were part of the hang. */
export function RowActionsMenu() {
  const [open, setOpen] = useState(false);

  return (
    <DropdownMenu open={open} onOpenChange={setOpen}>
      <DropdownMenuTrigger
        render={
          <Button
            variant="ghost"
            className="flex size-8 text-muted-foreground data-open:bg-muted"
            size="icon"
          />
        }
      >
        <HugeiconsIcon icon={MoreVerticalCircle01Icon} strokeWidth={2} />
        <span className="sr-only">Open menu</span>
      </DropdownMenuTrigger>
      {open ? (
        <DropdownMenuContent align="end" className="w-32">
          <DropdownMenuItem>Zero / home</DropdownMenuItem>
          <DropdownMenuItem>Apply preset</DropdownMenuItem>
          <DropdownMenuItem>Disable</DropdownMenuItem>
          <DropdownMenuSeparator />
          <DropdownMenuItem variant="destructive">Clear fault</DropdownMenuItem>
        </DropdownMenuContent>
      ) : null}
    </DropdownMenu>
  );
}
