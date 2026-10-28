import * as React from 'react';
import { useTestingStore } from '@/state/testingStore';
import { Button } from '@/components/ui/button';

type StopState = 'idle' | 'sending' | 'sent' | 'failed';

/**
 * One click sends the software stop (UI-01): runners are cancelled before the
 * request, and the result is shown, never swallowed.
 */
export function EStopButton() {
  const { disable } = useTestingStore();
  const [state, setState] = React.useState<StopState>('idle');
  const [error, setError] = React.useState<string | null>(null);

  const handleClick = async () => {
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

  const label = {
    idle: 'E-STOP',
    sending: 'STOPPING…',
    sent: 'STOP SENT',
    failed: 'STOP FAILED, USE HARDWARE E-STOP',
  }[state];

  return (
    <div className="space-y-1">
      <Button
        variant="destructive"
        size="lg"
        className="panel-brackets-fault w-full font-mono text-sm font-semibold uppercase tracking-[0.18em]"
        onClick={() => void handleClick()}
      >
        {label}
      </Button>
      {error ? (
        <p role="alert" className="text-xs text-destructive">
          {error}
        </p>
      ) : null}
    </div>
  );
}
