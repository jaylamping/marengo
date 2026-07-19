import * as React from 'react';
import { useCompoundStore } from '@/state/compoundStore';
import { useTestingStore } from '@/state/testingStore';
import { useRobotStore } from '@/state/robotStore';
import { COMPOUND_TEST_PRESETS } from '@/data/compound-tests';
import { fetchConfigSnapshot, ConfigSnapshotDto } from '@/lib/config-api';
import {
  jointsSettled,
  presetSegmentCount,
  segmentDurationSec,
  segmentTargets,
  encodeNativeWaveCommandName,
  nativeWaveDurationSec,
  WAYPOINT_SETTLE_HOLD_SEC,
} from '@/lib/compound-runner';
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

/** Zero gains → Pi clears overrides and uses arm_2dof_right control.yaml impedance. */
const CONFIG_GAINS = { kp: 0, kd: 0, ki: 0, fc: 0 };

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

  // Raise keyframes, then optional Berthier in-loop wave (no endpoint holds).
  const runnerRef = React.useRef<{
    intervalId: number | null;
    segmentIndex: number;
    segmentStartedAtMs: number;
    activeTargets: Record<string, number>;
    posting: boolean;
    settledSinceMs: number | null;
    phase: 'raise' | 'wave';
    waveEndsAtMs: number | null;
  }>({
    intervalId: null,
    segmentIndex: 0,
    segmentStartedAtMs: 0,
    activeTargets: {},
    posting: false,
    settledSinceMs: null,
    phase: 'raise',
    waveEndsAtMs: null,
  });

  React.useEffect(() => {
    fetchConfigSnapshot().then(setConfig);
  }, []);

  const postPositionBatch = React.useCallback(async (joints: MitJointCommand[]) => {
    await postTestingMitCommandBatch(
      create(MitCommandBatchSchema, {
        timestampMs: BigInt(Date.now()),
        mode: ControlMode.POSITION,
        joints,
      })
    );
  }, []);

  const returnHome = React.useCallback(
    async (jointNames: string[]) => {
      const joints = jointNames.map((name) =>
        create(MitJointCommandSchema, {
          name,
          ...CONFIG_GAINS,
          position: 0,
          velocity: 0,
          torqueFf: 0,
        })
      );
      if (!dryRun && joints.length > 0) {
        await postPositionBatch(joints);
      }
    },
    [dryRun, postPositionBatch]
  );

  const stopRunner = React.useCallback(
    (opts?: { returnHome?: boolean }) => {
      if (runnerRef.current.intervalId !== null) {
        window.clearInterval(runnerRef.current.intervalId);
        runnerRef.current.intervalId = null;
      }
      setIsRunning(false);
      setProgress(0);
      const shouldHome = opts?.returnHome !== false;
      const preset = COMPOUND_TEST_PRESETS.find((p) => p.id === selectedPresetId);
      if (shouldHome && preset) {
        void returnHome(preset.joints);
      }
    },
    [setIsRunning, setProgress, selectedPresetId, returnHome]
  );

  // Stop runner on unmount (do not return-home on unmount — only explicit Stop)
  React.useEffect(() => {
    return () => {
      if (runnerRef.current.intervalId !== null) {
        window.clearInterval(runnerRef.current.intervalId);
        runnerRef.current.intervalId = null;
      }
      setIsRunning(false);
    };
  }, [setIsRunning]);

  // Fault / disable: stop playback but do NOT returnHome (re-enable + slam).
  React.useEffect(() => {
    if (isRunning && operationalMode !== 'ACTIVE' && !dryRun) {
      stopRunner({ returnHome: false });
    }
  }, [isRunning, operationalMode, dryRun, stopRunner]);

  const startRunner = React.useCallback(() => {
    if (!selectedPresetId) return;
    const preset = COMPOUND_TEST_PRESETS.find((p) => p.id === selectedPresetId);
    if (!preset) return;

    const segmentCount = presetSegmentCount(preset.keyframes);
    if (segmentCount === 0) return;

    const limits: Record<string, { min: number; max: number }> = {};
    for (const jointName of preset.joints) {
      const motorConfig = config?.motors.find((m) => m.joint === jointName);
      limits[jointName] = {
        min: motorConfig?.bench.position_lower_rad ?? -Math.PI,
        max: motorConfig?.bench.position_upper_rad ?? Math.PI,
      };
    }

    const postSegment = async (
      segmentIndex: number,
      nextTrims: Record<string, number>,
      dry: boolean
    ) => {
      const targets = segmentTargets(
        preset.joints,
        preset.keyframes,
        segmentIndex,
        nextTrims,
        limits
      );
      runnerRef.current.activeTargets = targets;
      runnerRef.current.segmentIndex = segmentIndex;
      runnerRef.current.segmentStartedAtMs = Date.now();
      runnerRef.current.settledSinceMs = null;
      const commands = Object.entries(targets).map(([name, position]) =>
        create(MitJointCommandSchema, {
          name,
          ...CONFIG_GAINS,
          position,
          velocity: 0,
          torqueFf: 0,
        })
      );
      if (!dry && commands.length > 0) {
        await postPositionBatch(commands);
      }
    };

    const postNativeWave = async (dry: boolean, speedMult: number) => {
      const wave = preset.nativeWave;
      if (!wave) return;
      const halfScaled = Math.max(0.05, wave.halfPeriodSec / speedMult);
      const durationSec = nativeWaveDurationSec({
        ...wave,
        halfPeriodSec: halfScaled,
      });
      runnerRef.current.phase = 'wave';
      runnerRef.current.segmentStartedAtMs = Date.now();
      runnerRef.current.waveEndsAtMs = Date.now() + durationSec * 1000;
      runnerRef.current.activeTargets = {
        ...runnerRef.current.activeTargets,
        [wave.joint]: wave.maxRad,
      };
      const cmd = create(MitJointCommandSchema, {
        name: encodeNativeWaveCommandName(wave, speedMult),
        ...CONFIG_GAINS,
        position: wave.minRad,
        velocity: 0,
        torqueFf: 0,
      });
      if (!dry) {
        await postPositionBatch([cmd]);
      }
    };

    runnerRef.current.segmentIndex = 0;
    runnerRef.current.activeTargets = {};
    runnerRef.current.posting = false;
    runnerRef.current.phase = 'raise';
    runnerRef.current.waveEndsAtMs = null;
    setIsRunning(true);

    void postSegment(0, useCompoundStore.getState().trims, useTestingStore.getState().dryRun);

    runnerRef.current.intervalId = window.setInterval(() => {
      if (runnerRef.current.posting) return;

      const currentCompoundState = useCompoundStore.getState();
      const liveRobot = useRobotStore.getState().robotState;
      const currentSpeedMultiplier = Math.max(0.25, currentCompoundState.speedMultiplier);
      const currentLoop = currentCompoundState.loop;
      const currentTrims = currentCompoundState.trims;
      const currentDryRun = useTestingStore.getState().dryRun;
      const nowMs = Date.now();

      const measured: Record<string, number | undefined> = {};
      const measuredVel: Record<string, number | undefined> = {};
      for (const jointName of preset.joints) {
        const j = liveRobot?.joints.find((x) => x.name === jointName);
        measured[jointName] = j?.position;
        measuredVel[jointName] = j?.velocity;
      }

      // --- Berthier in-loop wave phase ---
      if (runnerRef.current.phase === 'wave' && runnerRef.current.waveEndsAtMs !== null) {
        const waveStart = runnerRef.current.segmentStartedAtMs;
        const waveEnd = runnerRef.current.waveEndsAtMs;
        const waveDur = Math.max(1, waveEnd - waveStart);
        const raiseFrac = 0.2;
        setProgress(Math.min(raiseFrac + (1 - raiseFrac) * ((nowMs - waveStart) / waveDur), 1));
        if (nowMs < waveEnd) return;
        if (currentLoop && preset.nativeWave) {
          // Do NOT re-post wave — restarting resets cosine phase to min and jerks the arm.
          // Pi wave still has remaining cycles; only roll the UI timer.
          const halfScaled = Math.max(0.05, preset.nativeWave.halfPeriodSec / currentSpeedMultiplier);
          const extendSec = nativeWaveDurationSec({
            ...preset.nativeWave,
            halfPeriodSec: halfScaled,
          });
          runnerRef.current.segmentStartedAtMs = nowMs;
          runnerRef.current.waveEndsAtMs = nowMs + extendSec * 1000;
          return;
        }
        stopRunner({ returnHome: true });
        return;
      }

      // --- Raise / keyframe phase ---
      const seg = runnerRef.current.segmentIndex;
      const dwellSec = segmentDurationSec(preset.keyframes, seg) / currentSpeedMultiplier;
      const elapsedSec = (nowMs - runnerRef.current.segmentStartedAtMs) / 1000;
      const dwellDone = elapsedSec >= dwellSec;

      const advanceMode = preset.advance ?? 'settle';
      const positionVelOk = jointsSettled(
        measured,
        runnerRef.current.activeTargets,
        undefined,
        measuredVel
      );
      if (positionVelOk) {
        if (runnerRef.current.settledSinceMs === null) {
          runnerRef.current.settledSinceMs = nowMs;
        }
      } else {
        runnerRef.current.settledSinceMs = null;
      }
      const settleHoldSec =
        runnerRef.current.settledSinceMs === null
          ? 0
          : (nowMs - runnerRef.current.settledSinceMs) / 1000;
      const settled =
        positionVelOk && settleHoldSec >= WAYPOINT_SETTLE_HOLD_SEC;
      const mayAdvance = advanceMode === 'timed' ? dwellDone : dwellDone && settled;

      setProgress(
        Math.min(
          (seg + (mayAdvance ? 1 : elapsedSec / Math.max(dwellSec, 1e-6))) /
            Math.max(segmentCount + (preset.nativeWave ? 1 : 0), 1),
          1
        )
      );

      if (!mayAdvance) return;

      let next = seg + 1;
      if (next >= segmentCount) {
        if (preset.nativeWave) {
          runnerRef.current.posting = true;
          void postNativeWave(currentDryRun, currentSpeedMultiplier).finally(() => {
            runnerRef.current.posting = false;
          });
          return;
        }
        if (!currentLoop) {
          stopRunner({ returnHome: true });
          return;
        }
        next = 0;
      }

      runnerRef.current.posting = true;
      void postSegment(next, currentTrims, currentDryRun).finally(() => {
        runnerRef.current.posting = false;
      });
    }, 100);
  }, [selectedPresetId, config, setProgress, setIsRunning, stopRunner, postPositionBatch]);

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
                    <Badge key={j} variant="secondary" className="text-xs font-normal">
                      {j}
                    </Badge>
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
            <Button
              variant="outline"
              size="sm"
              onClick={() => {
                stopRunner({ returnHome: false });
                setSelectedPresetId(null);
              }}
            >
              Back to Presets
            </Button>
          </CardHeader>
          <CardContent className="space-y-8 pt-6">
            <div className="space-y-4">
              <h4 className="text-sm font-semibold uppercase tracking-wider text-muted-foreground">
                Joint Trims
              </h4>
              <div className="grid grid-cols-1 md:grid-cols-2 gap-6">
                {selectedPreset.joints.map((jointName) => {
                  const trim = trims[jointName] || 0;
                  const jointState = robotState?.joints.find((j) => j.name === jointName);
                  const currentPos = jointState?.position ?? 0;
                  return (
                    <div
                      key={jointName}
                      className="space-y-2 bg-muted/30 p-3 rounded-lg border border-border/50"
                    >
                      <div className="flex justify-between text-xs font-medium">
                        <span>{jointName}</span>
                        <span className="text-muted-foreground">
                          Pos: {currentPos.toFixed(2)} rad | Trim: {trim > 0 ? '+' : ''}
                          {trim.toFixed(2)}
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
              <h4 className="text-sm font-semibold uppercase tracking-wider text-muted-foreground">
                Playback Controls
              </h4>
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
                    <label htmlFor="loop-toggle" className="text-sm font-medium cursor-pointer">
                      Loop
                    </label>
                  </div>
                  <div className="flex items-center gap-2">
                    <Checkbox
                      id="dry-run-toggle"
                      checked={dryRun}
                      onCheckedChange={toggleDryRun}
                    />
                    <label htmlFor="dry-run-toggle" className="text-sm font-medium cursor-pointer">
                      Dry Run
                    </label>
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
                  <Button
                    variant="destructive"
                    className="flex-1 font-bold"
                    onClick={() => stopRunner({ returnHome: true })}
                  >
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
