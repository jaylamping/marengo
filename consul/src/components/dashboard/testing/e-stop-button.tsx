import * as React from 'react';
import { useTestingStore } from '@/state/testingStore';
import { Button } from '@/components/ui/button';

export function EStopButton() {
  const { disable } = useTestingStore();
  const [confirm, setConfirm] = React.useState(false);

  const handleClick = () => {
    if (!confirm) {
      setConfirm(true);
      setTimeout(() => setConfirm(false), 3000);
    } else {
      disable();
      setConfirm(false);
    }
  };

  return (
    <Button 
      variant="destructive" 
      size="lg" 
      className="w-full text-lg font-bold"
      onClick={handleClick}
    >
      {confirm ? 'CONFIRM E-STOP' : 'E-STOP'}
    </Button>
  );
}
