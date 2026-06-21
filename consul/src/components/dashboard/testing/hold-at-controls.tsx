import * as React from 'react';
import { useTestingStore } from '@/state/testingStore';
import { Slider } from '@/components/ui/slider';
import { Button } from '@/components/ui/button';
import { Checkbox } from '@/components/ui/checkbox';
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card';

export function HoldAtControls() {
  const { setpointRad, setSetpoint, startTest, stopTest, dryRun, toggleDryRun, mode, setMode } = useTestingStore();

  return (
    <Card>
      <CardHeader><CardTitle>Hold Controls</CardTitle></CardHeader>
      <CardContent className="space-y-4">
        <div className="flex gap-2">
          <Button variant={mode === 'hold' ? 'default' : 'outline'} onClick={() => setMode('hold')}>Hold</Button>
          <Button variant={mode === 'sweep' ? 'default' : 'outline'} onClick={() => setMode('sweep')}>Sweep</Button>
        </div>
        <div className="space-y-1">
          <div className="flex justify-between text-xs">
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
        <div className="flex items-center gap-2">
          <Checkbox checked={dryRun} onCheckedChange={toggleDryRun} />
          <span className="text-sm">Dry Run</span>
        </div>
        <div className="flex gap-2">
          <Button onClick={startTest}>Start Hold</Button>
          <Button variant="destructive" onClick={stopTest}>Stop</Button>
        </div>
      </CardContent>
    </Card>
  );
}
