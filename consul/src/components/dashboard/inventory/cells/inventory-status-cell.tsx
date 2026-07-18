import { Badge } from '@/components/ui/badge';
import { HugeiconsIcon } from '@hugeicons/react';
import {
  CheckmarkCircle01Icon,
  Loading03Icon,
} from '@hugeicons/core-free-icons';

import { isHealthyStatus } from '@/components/dashboard/inventory/utils';

type InventoryStatusCellProps = {
  status: string;
};

export function InventoryStatusCell({ status }: InventoryStatusCellProps) {
  return (
    <Badge variant="outline" className="px-1.5 text-muted-foreground">
      {isHealthyStatus(status) ? (
        <HugeiconsIcon
          icon={CheckmarkCircle01Icon}
          strokeWidth={2}
          className="fill-ok"
        />
      ) : status === 'Tuning' ? (
        <HugeiconsIcon icon={Loading03Icon} strokeWidth={2} />
      ) : status === 'Fault' ? (
        <HugeiconsIcon
          icon={Loading03Icon}
          strokeWidth={2}
          className="text-destructive"
        />
      ) : null}
      {status}
    </Badge>
  );
}
