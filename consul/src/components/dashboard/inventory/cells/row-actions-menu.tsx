import { useState } from 'react';
import { useNavigate } from 'react-router-dom';

import { Button } from '@/components/ui/button';
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuTrigger,
} from '@/components/ui/dropdown-menu';
import { HugeiconsIcon } from '@hugeicons/react';
import { MoreVerticalCircle01Icon } from '@hugeicons/core-free-icons';

/**
 * Row overflow menu — Telemetry is read-only; commissioning lives on Hardware.
 * Menu content mounts only while open (avoid N always-on menus).
 */
export function RowActionsMenu() {
  const [open, setOpen] = useState(false);
  const navigate = useNavigate();

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
        <DropdownMenuContent align="end" className="w-40">
          <DropdownMenuItem
            onClick={() => {
              setOpen(false);
              void navigate('/hardware');
            }}
          >
            Open on Hardware
          </DropdownMenuItem>
        </DropdownMenuContent>
      ) : null}
    </DropdownMenu>
  );
}
