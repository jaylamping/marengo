import * as React from 'react';
import { useRobotStore } from '@/state/robotStore';
import { useTestingStore } from '@/state/testingStore';
import { postHomeCommand } from '@/lib/gateway-api';
import { Button } from '@/components/ui/button';
import { Badge } from '@/components/ui/badge';

export function EnableDisableButtons() {
  const { operationalMode } = useRobotStore();
  const { enable, disable } = useTestingStore();
  const [homing, setHoming] = React.useState(false);

  const handleHome = async () => {
    setHoming(true);
    try {
      await postHomeCommand();
    } catch (e) {
      console.error('Home command failed:', e);
    } finally {
      setHoming(false);
    }
  };

  return (
    <div className="flex items-center gap-4">
      <Badge variant={operationalMode === 'ACTIVE' ? 'default' : 'outline'}>
        {operationalMode || 'DISABLED'}
      </Badge>
      <Button variant="outline" onClick={handleHome} disabled={homing}>
        {homing ? 'Homing...' : 'Home'}
      </Button>
      <Button disabled={operationalMode !== 'READY'} onClick={enable}>Enable</Button>
      <Button variant="destructive" onClick={disable}>Disable</Button>
    </div>
  );
}
