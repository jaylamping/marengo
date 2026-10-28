import * as React from 'react';

import { Button } from '@/components/ui/button';
import { useTestingStore } from '@/state/testingStore';

type StopState = 'idle' | 'sending' | 'sent' | 'failed';

/**
 * Software Disable on every route (UI-02): one click cancels any browser
 * runner and sends the stop, which needs no gateway credential.
 */
export function HeaderDisableButton() {
  const disable = useTestingStore((state) => state.disable);
  const [state, setState] = React.useState<StopState>('idle');
  const [error, setError] = React.useState<string | null>(null);

  const onClick = async () => {
    setState('sending');
    setError(null);
    try {
      await disable();
      setState('sent');
    } catch (cause) {
      setState('failed');
      setError(cause instanceof Error ? cause.message : String(cause));
    }
  };

  return (
    <Button
      type="button"
      size="sm"
      variant="destructive"
      className="font-mono text-xs uppercase tracking-[0.14em]"
      data-testid="header-disable"
      title={error ?? 'Software stop: disable every drive. Not a substitute for the hardware E-stop.'}
      onClick={() => void onClick()}
    >
      {state === 'sending' ? 'Disabling…' : state === 'failed' ? 'Disable failed' : 'Disable'}
    </Button>
  );
}
