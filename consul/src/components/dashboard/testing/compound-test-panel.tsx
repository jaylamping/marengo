import * as React from 'react';
import { useCompoundStore } from '@/state/compoundStore';
import { useTestingStore } from '@/state/testingStore';
import { useRobotStore } from '@/state/robotStore';
import { COMPOUND_TEST_PRESETS } from '@/data/compound-tests';
import { fetchConfigSnapshot, ConfigSnapshotDto } from '@/lib/config-api';
import { computeTickCommand } from '@/lib/compound-runner';
import { Card, CardContent, CardHeader, CardTitle, CardDescription } from '@/components/ui/card';
import { Button } from '@/components/ui/button';
import { Slider } from '@/components/ui/slider';
import { Checkbox } from '@/components/ui/checkbox';
import { Badge } from '@/components/ui/badge';
import { create } from '@bufbuild/protobuf';
import {
  ControlMode,
  MitCommandBatchSchema,
  MitJointCommandSchema,
  type MitJointCommand,
} from '@/gen/marengo/v1/marengo_pb';
import { postTestingMitCommandBatch } from '@/lib/gateway-api';
import { dashboardPanelCardClassName } from '@/components/dashboard/layout/constants';

export function CompoundTestPanel() {
  const {
    selectedPresetId,
    setSelectedPresetId,
    trims,
    setTrim,
    speedMultiplier,
    setSpeedMultiplier,
    loop,
    setLoop,
    progress,
    setProgress,
    isRunning,
    setIsRunning,
  } = useCompoundStore();

  const { dryRun, toggleDryRun } = useTestingStore();
  const { robotState, operationalMode } = useRobotStore();
  const [config, setConfig] = React.useState<ConfigSnapshotDto | null>(null);

  // For runner state
  const runnerRef = React.useRef<{
    intervalId: number | null;
    startTimeMs: number;
    initialPositions: Record<string, number>;
  }>({
    intervalId: null,
    startTimeMs: 0,
    initialPositions: {},
  });

  React.useEffect(() => {
    fetchConfigSnapshot().then(setConfig);
  }, []);

  const stopRunner = React.useCallback(() => {
    if (runnerRef.current.intervalId !== null) {
      window.clearInterval(runnerRef.current.intervalId);
      runnerRef.current.intervalId = null;
    }
    setIsRunning(false);
    setProgress(0);
  }, [setIsRunning, setProgress]);

  // Stop runner on unmount
  React.useEffect(() => {
    return () => stopRunner();
  }, [stopRunner]);

  // Stop runner if operational mode is no longer ACTIVE (unless in dry run)
  React.useEffect(() => {
    if (isRunning && operationalMode !== 'ACTIVE' && !dryRun) {
      stopRunner();
    }
  }, [isRunning, operationalMode, dryRun, stopRunner]);

  const startRunner = React.useCallback(() => {
    if (!selectedPresetId) return;
    const preset = COMPOUND_TEST_PRESETS.find((p) => p.id === selectedPresetId);
    if (!preset) return;

    // Capture initial positions
    const initialPositions: Record<string, number> = {};
    for (const jointName of preset.joints) {
      const jointState = robotState?.joints.find((j) => j.name === jointName);
      initialPositions[jointName] = jointState?.position ?? 0;
    }

    runnerRef.current.startTimeMs = Date.now();
    runnerRef.current.initialPositions = initialPositions;
    setIsRunning(true);

    const totalDuration = Math.max(
      ...Object.values(preset.keyframes).map((kfs) =>
        kfs.reduce((sum, kf) => sum + kf.durationSec, 0)
      )
    );

    runnerRef.current.intervalId = window.setInterval(async () => {
      // Read latest values from stores to avoid stale closures
      const currentCompoundState = useCompoundStore.getState();
      const currentTestingState = useTestingStore.getState();
      
      const currentSpeedMultiplier = currentCompoundState.speedMultiplier;
      const currentLoop = currentCompoundState.loop;
      const currentTrims = currentCompoundState.trims;
      const currentDryRun = currentTestingState.dryRun;
      const currentGains = currentTestingState.gains;

      const now = Date.now();
      const elapsedSec = ((now - runnerRef.current.startTimeMs) / 1000) * currentSpeedMultiplier;

      let allFinished = true;
      const commands: MitJointCommand[] = [];

      for (const jointName of preset.joints) {
        const kfs = preset.keyframes[jointName] || [];
        const initialRad = runnerRef.current.initialPositions[jointName] || 0;
        const trim = currentTrims[jointName] || 0;
        
        const motorConfig = config?.motors.find((m) => m.joint === jointName);
        const minLimit = motorConfig?.bench.position_lower_rad ?? -Math.PI;
        const maxLimit = motorConfig?.bench.position_upper_rad ?? Math.PI;

        const { position: finalPosition, isFinished } = computeTickCommand({
          jointName,
          keyframes: kfs,
          initialRad,
          elapsedSec,
          loop: currentLoop,
          trim,
          minLimit,
          maxLimit,
        });

        if (!isFinished) allFinished = false;

        const gain = currentGains[jointName] || { kp: 0, kd: 0, ki: 0, fc: 0 };
        commands.push(
          create(MitJointCommandSchema, {
            name: jointName,
            kp: gain.kp,
            kd: gain.kd,
            ki: gain.ki,
            fc: gain.fc,
            position: finalPosition,
            velocity: 0,
            torqueFf: 0,
          })
        );
      }

      if (totalDuration > 0) {
        let currentProgress = elapsedSec / totalDuration;
        if (currentLoop) currentProgress = currentProgress % 1;
        setProgress(Math.min(currentProgress, 1));
      } else {
        setProgress(1);
      }

      if (!currentDryRun && commands.length > 0) {
        await postTestingMitCommandBatch(
          create(MitCommandBatchSchema, {
            timestampMs: BigInt(Date.now()),
            mode: ControlMode.POSITION,
            joints: commands,
          })
        );
      }

      if (allFinished && !currentLoop) {
        stopRunner();
      }
    }, 100); // ~10 Hz
  }, [selectedPresetId, robotState, config, setProgress, setIsRunning, stopRunner]);

  const selectedPreset = COMPOUND_TEST_PRESETS.find((p) => p.id === selectedPresetId);

  return (
    <div className="space-y-4">
      {!selectedPreset ? (
        <div className="grid grid-cols-1 md:grid-cols-2 gap-4">
          {COMPOUND_TEST_PRESETS.map((preset) => (
            <Card
              key={preset.id}
              variant="panel"
              className={`${dashboardPanelCardClassName} cursor-pointer hover:border-primary transition-colors`}
              onClick={() => setSelectedPresetId(preset.id)}
            >
              <CardHeader>
                <CardTitle className="text-lg">{preset.name}</CardTitle>
                <CardDescription>{preset.description}</CardDescription>
              </CardHeader>
              <CardContent>
                <div className="flex flex-wrap gap-2">
                  {preset.joints.map((j) => (
                    <Badge key={j} variant="secondary" className="text-xs font-normal">{j}</Badge>
                  ))}
                </div>
              </CardContent>
            </Card>
          ))}
        </div>
      ) : (
        <Card variant="panel" className={dashboardPanelCardClassName}>
          <CardHeader className="flex flex-row items-center justify-between space-y-0 pb-4 border-b border-border/50">
            <div>
              <CardTitle className="text-xl">{selectedPreset.name}</CardTitle>
              <CardDescription className="mt-1">{selectedPreset.description}</CardDescription>
            </div>
            <Button variant="outline" size="sm" onClick={() => { stopRunner(); setSelectedPresetId(null); }}>
              Back to Presets
            </Button>
          </CardHeader>
          <CardContent className="space-y-8 pt-6">
            <div className="space-y-4">
              <h4 className="text-sm font-semibold uppercase tracking-wider text-muted-foreground">Joint Trims</h4>
              <div className="grid grid-cols-1 md:grid-cols-2 gap-6">
                {selectedPreset.joints.map((jointName) => {
                  const trim = trims[jointName] || 0;
                  const jointState = robotState?.joints.find((j) => j.name === jointName);
                  const currentPos = jointState?.position ?? 0;
                  return (
                    <div key={jointName} className="space-y-2 bg-muted/30 p-3 rounded-lg border border-border/50">
                      <div className="flex justify-between text-xs font-medium">
                        <span>{jointName}</span>
                        <span className="text-muted-foreground">
                          Pos: {currentPos.toFixed(2)} rad | Trim: {trim > 0 ? '+' : ''}{trim.toFixed(2)}
                        </span>
                      </div>
                      <Slider
                        value={[trim]}
                        min={-0.5}
                        max={0.5}
                        step={0.01}
                        onValueChange={(v) => setTrim(jointName, v[0])}
                      />
                    </div>
                  );
                })}
              </div>
            </div>

            <div className="space-y-4">
              <h4 className="text-sm font-semibold uppercase tracking-wider text-muted-foreground">Playback Controls</h4>
              <div className="grid grid-cols-1 md:grid-cols-2 gap-6 items-end">
                <div className="space-y-2">
                  <div className="flex justify-between text-xs font-medium">
                    <span>Speed Multiplier</span>
                    <span>{speedMultiplier.toFixed(2)}x</span>
                  </div>
                  <Slider
                    value={[speedMultiplier]}
                    min={0.25}
                    max={2.0}
                    step={0.25}
                    onValueChange={(v) => setSpeedMultiplier(v[0])}
                  />
                </div>
                
                <div className="flex items-center gap-6 pb-1">
                  <div className="flex items-center gap-2">
                    <Checkbox
                      id="loop-toggle"
                      checked={loop}
                      onCheckedChange={(c) => setLoop(!!c)}
                    />
                    <label htmlFor="loop-toggle" className="text-sm font-medium cursor-pointer">Loop</label>
                  </div>
                  <div className="flex items-center gap-2">
                    <Checkbox
                      id="dry-run-toggle"
                      checked={dryRun}
                      onCheckedChange={toggleDryRun}
                    />
                    <label htmlFor="dry-run-toggle" className="text-sm font-medium cursor-pointer">Dry Run</label>
                  </div>
                </div>
              </div>
            </div>

            <div className="space-y-3 pt-2">
              {!robotState && (
                <div className="text-xs text-muted-foreground italic">
                  No live telemetry — dry-run preview starts from home (0 rad)
                </div>
              )}
              <div className="h-2 bg-muted rounded-full overflow-hidden">
                <div
                  className="h-full bg-primary transition-all duration-100 ease-linear"
                  style={{ width: `${progress * 100}%` }}
                />
              </div>
              <div className="flex gap-3">
                {isRunning ? (
                  <Button variant="destructive" className="flex-1 font-bold" onClick={stopRunner}>
                    Stop
                  </Button>
                ) : (
                  <Button className="flex-1 font-bold" onClick={startRunner}>
                    Start
                  </Button>
                )}
              </div>
            </div>
          </CardContent>
        </Card>
      )}
    </div>
  );
}
