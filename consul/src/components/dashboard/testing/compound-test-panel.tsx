import * as React from 'react';
import { HugeiconsIcon } from '@hugeicons/react';
import { ArrowLeft01Icon } from '@hugeicons/core-free-icons';
import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from '@/components/ui/card';
import { Badge } from '@/components/ui/badge';
import { Button } from '@/components/ui/button';
import { Checkbox } from '@/components/ui/checkbox';
import { Slider } from '@/components/ui/slider';
import { dashboardPanelCardClassName } from '@/components/dashboard/layout/constants';
import { RecordMovementPanel } from '@/components/dashboard/testing/record-movement-panel';
import {
  COMPOUND_TEST_PRESETS,
  compoundPresetById,
} from '@/data/compound-tests';
import { useCompoundPlayback } from '@/hooks/use-compound-playback';
import { useConfigSnapshot } from '@/hooks/use-config-snapshot';
import { liveFingerprint, resolvePlayablePreset } from '@/lib/teach-transit';
import { useCompoundStore } from '@/state/compoundStore';
import { useHostMetricsStore } from '@/state/hostMetricsStore';
import { useRobotStore } from '@/state/robotStore';
import { captureIsRecording, useTeachStore } from '@/state/teachStore';
import { useTestingStore } from '@/state/testingStore';

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
    isRunning,
  } = useCompoundStore();
  const { dryRun, toggleDryRun } = useTestingStore();
  const robotState = useRobotStore((state) => state.robotState);
  const operationalMode = useRobotStore((state) => state.operationalMode);
  const connected = useRobotStore((state) => state.connected);
  const capture = useTeachStore((state) => state.capture);
  const overlays = useTeachStore((state) => state.overlays);
  const liveCalibrationEpoch = useTeachStore(
    (state) => state.liveCalibrationEpoch,
  );
  const { data: config = null } = useConfigSnapshot();
  const { overlayBlockReason, setOverlayBlockReason, startRunner, stopRunner } =
    useCompoundPlayback();
  const [recordSectionOpen, setRecordSectionOpen] = React.useState(false);
  const recording = captureIsRecording(capture);

  const closePresetDetail = React.useCallback(() => {
    useTeachStore.getState().cancelRecording();
    stopRunner({ returnHome: false });
    setOverlayBlockReason(null);
    setRecordSectionOpen(false);
    setSelectedPresetId(null);
  }, [setOverlayBlockReason, setSelectedPresetId, stopRunner]);

  React.useEffect(() => {
    if (selectedPresetId !== null) return;
    useTeachStore.getState().cancelRecording();
    setRecordSectionOpen(false);
    setOverlayBlockReason(null);
  }, [selectedPresetId, setOverlayBlockReason]);

  const selectedBase = selectedPresetId
    ? compoundPresetById(selectedPresetId)
    : undefined;
  const playableSelected = React.useMemo(() => {
    if (!selectedBase || !selectedPresetId) return null;
    const liveFingerprintValue = liveFingerprint(
      config?.profile ?? 'arm_4dof_right',
      selectedBase.joints,
      useHostMetricsStore.getState().piMetrics?.build,
    );
    return resolvePlayablePreset(
      selectedBase,
      overlays[selectedPresetId],
      liveFingerprintValue,
      liveCalibrationEpoch,
    );
  }, [
    config?.profile,
    liveCalibrationEpoch,
    overlays,
    selectedBase,
    selectedPresetId,
  ]);
  const selectedPreset = playableSelected?.preset;

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
                  {preset.joints.map((joint) => (
                    <Badge
                      key={joint}
                      variant="secondary"
                      className="text-xs font-normal"
                    >
                      {joint}
                    </Badge>
                  ))}
                  {overlays[preset.id] ? (
                    <Badge className="text-xs font-normal">taught</Badge>
                  ) : null}
                </div>
              </CardContent>
            </Card>
          ))}
        </div>
      ) : (
        <Card variant="panel" className={dashboardPanelCardClassName}>
          <CardHeader className="space-y-0 pb-4 border-b border-border/50">
            <div className="flex items-start gap-2">
              <Button
                type="button"
                variant="panel"
                size="icon-sm"
                className="mt-0.5 shrink-0"
                aria-label="Back to presets"
                onClick={closePresetDetail}
              >
                <HugeiconsIcon icon={ArrowLeft01Icon} strokeWidth={2} />
              </Button>
              <div className="min-w-0 flex-1">
                <CardTitle className="text-xl">{selectedPreset.name}</CardTitle>
                <CardDescription className="mt-1">
                  {selectedPreset.description}
                </CardDescription>
                {playableSelected?.warning ? (
                  <p className="text-xs text-amber-600 dark:text-amber-400 mt-2">
                    {playableSelected.warning}
                  </p>
                ) : null}
              </div>
            </div>
          </CardHeader>
          <CardContent className="space-y-8 pt-6">
            <section className="space-y-4">
              <h4 className="text-sm font-semibold uppercase tracking-wider text-muted-foreground">
                Joint Trims
              </h4>
              <div className="grid grid-cols-1 md:grid-cols-2 gap-6">
                {selectedPreset.joints.map((joint) => {
                  const trim = trims[joint] || 0;
                  const position =
                    robotState?.joints.find((state) => state.name === joint)
                      ?.position ?? 0;
                  return (
                    <div
                      key={joint}
                      className="space-y-2 bg-muted/30 p-3 rounded-lg border border-border/50"
                    >
                      <div className="flex justify-between text-xs font-medium">
                        <span>{joint}</span>
                        <span className="text-muted-foreground">
                          Pos: {position.toFixed(2)} rad | Trim:{' '}
                          {trim > 0 ? '+' : ''}
                          {trim.toFixed(2)}
                        </span>
                      </div>
                      <Slider
                        value={[trim]}
                        min={-0.5}
                        max={0.5}
                        step={0.01}
                        onValueChange={(value) => setTrim(joint, value[0])}
                      />
                    </div>
                  );
                })}
              </div>
            </section>
            <section className="space-y-3">
              <div className="flex items-center justify-between gap-3">
                <h4 className="text-sm font-semibold uppercase tracking-wider text-muted-foreground">
                  Record Movement
                </h4>
                <Button
                  size="sm"
                  variant={recordSectionOpen ? 'outline' : 'default'}
                  onClick={() => setRecordSectionOpen((open) => !open)}
                >
                  {recordSectionOpen
                    ? 'Hide Record Movement'
                    : 'Record Movement'}
                </Button>
              </div>
              <RecordMovementPanel
                presetId={selectedPreset.id}
                open={recordSectionOpen}
                onOpenChange={setRecordSectionOpen}
              />
            </section>
            <section className="space-y-4">
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
                    max={2}
                    step={0.25}
                    onValueChange={(value) => setSpeedMultiplier(value[0])}
                  />
                </div>
                <div className="flex items-center gap-6 pb-1">
                  <div className="flex items-center gap-2">
                    <Checkbox
                      id="loop-toggle"
                      checked={loop}
                      onCheckedChange={(checked) => setLoop(!!checked)}
                    />
                    <label
                      htmlFor="loop-toggle"
                      className="text-sm font-medium cursor-pointer"
                    >
                      Loop
                    </label>
                  </div>
                  <div className="flex items-center gap-2">
                    <Checkbox
                      id="dry-run-toggle"
                      checked={dryRun}
                      onCheckedChange={toggleDryRun}
                    />
                    <label
                      htmlFor="dry-run-toggle"
                      className="text-sm font-medium cursor-pointer"
                    >
                      Dry Run
                    </label>
                  </div>
                </div>
              </div>
            </section>
            <section className="space-y-3 pt-2">
              {!robotState ? (
                <p className="text-xs text-muted-foreground italic">
                  No live telemetry — dry-run preview starts from home (0 rad)
                </p>
              ) : null}
              <div className="h-2 bg-muted rounded-full overflow-hidden">
                <div
                  className="h-full bg-primary transition-all duration-100 ease-linear"
                  style={{ width: `${progress * 100}%` }}
                />
              </div>
              {recording ? (
                <p className="text-xs text-destructive">
                  Record Movement is active. Stop recording before starting this
                  compound test.
                </p>
              ) : null}
              {overlayBlockReason ? (
                <p className="text-xs text-destructive" role="alert">
                  {overlayBlockReason}
                </p>
              ) : null}
              {isRunning ? (
                <Button
                  variant="destructive"
                  className="flex-1 font-bold"
                  onClick={() => stopRunner({ returnHome: true })}
                >
                  Stop
                </Button>
              ) : (
                <Button
                  className="flex-1 font-bold"
                  onClick={startRunner}
                  disabled={
                    recording ||
                    (!dryRun && (operationalMode !== 'ACTIVE' || !connected))
                  }
                >
                  Start
                </Button>
              )}
            </section>
          </CardContent>
        </Card>
      )}
    </div>
  );
}
