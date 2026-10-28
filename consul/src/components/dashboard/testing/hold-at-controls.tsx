import * as React from 'react';
import { useCompoundStore } from '@/state/compoundStore';
import { useTestingStore } from '@/state/testingStore';
import { Slider } from '@/components/ui/slider';
import { Button } from '@/components/ui/button';
import { Checkbox } from '@/components/ui/checkbox';
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card';
import { dashboardPanelCardClassName } from '@/components/dashboard/layout/constants';

export function HoldAtControls() {
  const { setpointRad, setSetpoint, startTest, stopTest, returnHome, dryRun, toggleDryRun, isRunning } = useTestingStore();
  // Dry Run is shared with the compound runner; it is frozen while either runs (UI-03).
  const compoundRunning = useCompoundStore((state) => state.isRunning);
  const [error, setError] = React.useState<string | null>(null);

  // A refused POST is shown, never swallowed (UI-13).
  const run = (action: () => Promise<void>) => {
    setError(null);
    action().catch((cause: unknown) => {
      setError(cause instanceof Error ? cause.message : String(cause));
    });
  };

  return (
    <Card variant="panel" className={dashboardPanelCardClassName}>
      <CardHeader className="pb-3"><CardTitle>Hold Controls</CardTitle></CardHeader>
      <CardContent className="space-y-6">
        <div className="space-y-2">
          <div className="flex justify-between text-sm font-medium">
            <span>Setpoint (rad)</span>
            <span>{setpointRad.toFixed(2)}</span>
          </div>
          <Slider
            value={[setpointRad]}
            min={-Math.PI}
            max={Math.PI}
            step={0.01}
            onValueChange={(v) => setSetpoint(v[0])}
          />
        </div>
        <div className="flex items-center justify-between">
          <div className="flex items-center gap-2">
            <Checkbox
              id="dry-run-manual"
              checked={dryRun}
              disabled={isRunning || compoundRunning}
              onCheckedChange={toggleDryRun}
            />
            <label htmlFor="dry-run-manual" className="text-sm font-medium cursor-pointer">Dry Run</label>
          </div>
          <div className="flex gap-2">
            {isRunning ? (
              <>
                <Button variant="outline" onClick={() => run(returnHome)}>Return Home</Button>
                <Button variant="destructive" onClick={() => run(stopTest)}>Stop</Button>
              </>
            ) : (
              <Button onClick={() => run(startTest)}>Start Hold</Button>
            )}
          </div>
        </div>
        {error ? (
          <p role="alert" className="text-xs text-destructive">
            {error}
          </p>
        ) : null}
      </CardContent>
    </Card>
  );
}
