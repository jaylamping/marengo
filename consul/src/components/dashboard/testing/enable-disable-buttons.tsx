import * as React from 'react';
import { useRobotStore } from '@/state/robotStore';
import { useTestingStore } from '@/state/testingStore';
import { Button } from '@/components/ui/button';
import { Badge } from '@/components/ui/badge';

export function EnableDisableButtons() {
  const { operationalMode } = useRobotStore();
  const { enable, disable } = useTestingStore();

  return (
    <div className="flex items-center gap-4">
      <Badge variant={operationalMode === 'ACTIVE' ? 'default' : 'outline'}>
        {operationalMode || 'DISABLED'}
      </Badge>
      <Button disabled={operationalMode !== 'READY'} onClick={enable}>Enable</Button>
      <Button variant="destructive" onClick={disable}>Disable</Button>
    </div>
  );
}
