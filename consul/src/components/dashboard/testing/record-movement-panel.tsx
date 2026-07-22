import * as React from 'react';

import { dashboardPanelCardClassName } from '@/components/dashboard/layout/constants';
import { Badge } from '@/components/ui/badge';
import { Button } from '@/components/ui/button';
import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from '@/components/ui/card';
import { Checkbox } from '@/components/ui/checkbox';
import { Slider } from '@/components/ui/slider';
import { COMPOUND_TEST_PRESETS } from '@/data/compound-tests';
import { useConfigSnapshot } from '@/hooks/use-config-snapshot';
import { overlayNeedsCalibrationAck } from '@/lib/teach-calibration';
import { canApplyLandmarks, extractLandmarks, samplesHaveMotion } from '@/lib/teach-record';
import { subscribeTeachSamples } from '@/lib/teach-sample-bus';
import { createTeachSession, liveFingerprint, materializeTaughtPreset } from '@/lib/teach-transit';
import { useCompoundStore } from '@/state/compoundStore';
import { useHostMetricsStore } from '@/state/hostMetricsStore';
import { useRobotStore } from '@/state/robotStore';
import { useTeachStore } from '@/state/teachStore';

interface RecordMovementPanelProps {
  presetId: string;
  open: boolean;
  onOpenChange: (open: boolean) => void;
}

export function RecordMovementPanel({
  presetId,
  open,
  onOpenChange,
}: RecordMovementPanelProps) {
  const base = COMPOUND_TEST_PRESETS.find((preset) => preset.id === presetId);
  const operationalMode = useRobotStore((state) => state.operationalMode);
  const connected = useRobotStore((state) => state.connected);
  const compoundRunning = useCompoundStore((state) => state.isRunning);
  const returnHomePending = useCompoundStore((state) => state.returnHomePending);
  const { data: config = null } = useConfigSnapshot();
  const piMetrics = useHostMetricsStore((state) => state.piMetrics);
  const recording = useTeachStore((state) => state.recording);
  const recordingPresetId = useTeachStore((state) => state.recordingPresetId);
  const draftPresetId = useTeachStore((state) => state.draftPresetId);
  const gravityArmed = useTeachStore((state) => state.gravityArmed);
  const samples = useTeachStore((state) => state.samples);
  const landmarks = useTeachStore((state) => state.landmarks);
  const cadenceScale = useTeachStore((state) => state.cadenceScale);
  const settleDwellSec = useTeachStore((state) => state.settleDwellSec);
  const lastError = useTeachStore((state) => state.lastError);
  const overlays = useTeachStore((state) => state.overlays);
  const liveCalibrationEpoch = useTeachStore((state) => state.liveCalibrationEpoch);
  const setGravityArmed = useTeachStore((state) => state.setGravityArmed);
  const startRecording = useTeachStore((state) => state.startRecording);
  const stopRecording = useTeachStore((state) => state.stopRecording);
  const appendSample = useTeachStore((state) => state.appendSample);
  const clearSamples = useTeachStore((state) => state.clearSamples);
  const setLandmarks = useTeachStore((state) => state.setLandmarks);
  const setLandmarkIncluded = useTeachStore((state) => state.setLandmarkIncluded);
  const setCadenceScale = useTeachStore((state) => state.setCadenceScale);
  const setSettleDwellSec = useTeachStore((state) => state.setSettleDwellSec);
  const setLastError = useTeachStore((state) => state.setLastError);
  const applyOverlay = useTeachStore((state) => state.applyOverlay);
  const clearOverlay = useTeachStore((state) => state.clearOverlay);
  const resetSession = useTeachStore((state) => state.resetSession);
  const markCalibrationChanged = useTeachStore((state) => state.markCalibrationChanged);
  const acknowledgeCalibration = useTeachStore((state) => state.acknowledgeCalibration);

  const joints = base?.joints ?? [];
  const isRecordingThisPreset = recording && recordingPresetId === presetId;
  const hasDraft = draftPresetId === presetId;
  const overlay = overlays[presetId];
  const needsCalAck = overlayNeedsCalibrationAck(overlay?.session, {
    liveCalibrationEpoch,
    ackedAtEpoch: overlay?.ackedAtEpoch ?? 0,
  });

  React.useEffect(() => {
    if (recordingPresetId && recordingPresetId !== presetId) {
      stopRecording();
    }
  }, [presetId, recordingPresetId, stopRecording]);

  React.useEffect(() => {
    if (!isRecordingThisPreset) return;
    return subscribeTeachSamples(appendSample);
  }, [isRecordingThisPreset, appendSample]);

  React.useEffect(() => {
    if (!isRecordingThisPreset) return;
    if (operationalMode !== 'ACTIVE' || !connected) {
      stopRecording();
      setLastError(
        !connected
          ? 'Record stopped because Chappe disconnected.'
          : 'Record stopped because the robot left ACTIVE.'
      );
    }
  }, [isRecordingThisPreset, operationalMode, connected, stopRecording, setLastError]);

  if (!base || !open) return null;

  const canRecord =
    gravityArmed &&
    connected &&
    operationalMode === 'ACTIVE' &&
    !compoundRunning &&
    !returnHomePending &&
    joints.length > 0;

  const startRecord = () => {
    if (!canRecord) {
      setLastError(
        returnHomePending
          ? 'Wait for return-home to settle before recording movement.'
          : 'Recording requires the gravity-comp armed checkbox, ACTIVE, and a stopped compound test.'
      );
      return;
    }
    clearSamples();
    setLandmarks([]);
    setLastError(null);
    startRecording(presetId);
  };

  const stopRecord = () => {
    stopRecording();
    if (!samplesHaveMotion(samples, joints)) {
      setLandmarks([]);
      setLastError('No motion in buffer. Nothing to apply.');
      return;
    }
    const extracted = extractLandmarks(samples, joints);
    setLandmarks(extracted);
    setLastError(
      canApplyLandmarks(extracted) ? null : 'Landmark extraction failed. Do not apply this draft.'
    );
  };

  const apply = () => {
    if (!hasDraft || !canApplyLandmarks(landmarks)) {
      setLastError('Cannot apply. Record movement for this preset first.');
      return;
    }
    const fingerprint = liveFingerprint(
      config?.profile ?? 'arm_4dof_right',
      joints,
      piMetrics?.build
    );
    const epoch = useTeachStore.getState().liveCalibrationEpoch;
    const session = createTeachSession(fingerprint, presetId, landmarks, {
      cadenceScale,
      settleDwellSec,
      calibrationEpoch: epoch,
    });
    const result = materializeTaughtPreset(session, base, fingerprint);
    if (!result.ok) {
      setLastError(`Apply refused: ${result.error}`);
      return;
    }
    applyOverlay(presetId, { session, ackedAtEpoch: epoch });
    const compound = useCompoundStore.getState();
    if (!compound.isRunning) compound.setLoop(result.preset.loop);
  };

  const download = () => {
    const payload = overlay ?? { landmarks, cadenceScale, settleDwellSec, samples: samples.length };
    const url = URL.createObjectURL(
      new Blob([JSON.stringify(payload, null, 2)], { type: 'application/json' })
    );
    const link = document.createElement('a');
    link.href = url;
    link.download = `teach-${presetId}-${Date.now()}.json`;
    link.click();
    URL.revokeObjectURL(url);
  };

  return (
    <Card variant="panel" className={dashboardPanelCardClassName}>
      <CardHeader className="flex flex-row items-start justify-between gap-4">
        <div>
          <CardTitle className="text-lg">Record Movement</CardTitle>
          <CardDescription>{base.teach.appliedDescription}</CardDescription>
        </div>
        <Button size="sm" variant="ghost" onClick={() => onOpenChange(false)}>
          Close
        </Button>
      </CardHeader>
      <CardContent className="space-y-6">
        <div className="rounded-md border border-border/60 bg-muted/20 p-3 text-xs text-muted-foreground space-y-2">
          <p>
            Record only in GravityComp. Testing commands post Position or Impedance and end
            GravityComp.
          </p>
          <div className="flex items-center gap-2">
            <Checkbox
              id={`gravity-armed-${presetId}`}
              checked={gravityArmed}
              onCheckedChange={(checked) => setGravityArmed(!!checked)}
              disabled={recording}
            />
            <label htmlFor={`gravity-armed-${presetId}`} className="cursor-pointer font-medium text-foreground">
              Gravity-comp armed and ACTIVE
            </label>
          </div>
          <div className="flex flex-wrap gap-2">
            <Badge variant="secondary">mode: {operationalMode ?? 'unknown'}</Badge>
            <Badge variant="secondary">samples: {hasDraft ? samples.length : 0}</Badge>
            {overlay ? <Badge>taught</Badge> : null}
            {needsCalAck ? <Badge variant="destructive">calibration ack needed</Badge> : null}
          </div>
        </div>

        {needsCalAck ? (
          <div className="rounded-md border border-amber-500/40 bg-amber-500/10 p-3 text-xs space-y-2">
            <p>Calibration changed after this overlay was applied.</p>
            <div className="flex flex-wrap gap-2">
              <Button size="sm" onClick={() => acknowledgeCalibration(presetId)}>
                Acknowledge and keep
              </Button>
              <Button
                size="sm"
                variant="outline"
                onClick={() => {
                  resetSession();
                  clearOverlay(presetId);
                }}
              >
                Reset overlay
              </Button>
            </div>
          </div>
        ) : null}

        <div className="flex flex-wrap gap-2">
          {isRecordingThisPreset ? (
            <Button variant="destructive" onClick={stopRecord}>
              Stop Record
            </Button>
          ) : (
            <Button onClick={startRecord} disabled={!canRecord || recording}>
              Record Movement
            </Button>
          )}
          <Button variant="outline" onClick={apply} disabled={recording || !hasDraft || !canApplyLandmarks(landmarks)}>
            Apply to {base.name}
          </Button>
          <Button variant="outline" onClick={download} disabled={!hasDraft && !overlay}>
            Download
          </Button>
          <Button
            variant="secondary"
            onClick={markCalibrationChanged}
            disabled={!overlay || recording}
            title="Use after set-zero of teach joints"
          >
            I set-zero&apos;d
          </Button>
          <Button
            variant="ghost"
            onClick={() => {
              resetSession();
              clearOverlay(presetId);
            }}
          >
            Reset
          </Button>
        </div>

        {compoundRunning ? <p className="text-xs text-muted-foreground">Stop the compound test before recording.</p> : null}
        {returnHomePending ? <p className="text-xs text-muted-foreground">Return-home is in progress.</p> : null}
        {lastError ? <p className="text-sm text-destructive" role="alert">{lastError}</p> : null}

        <div className="space-y-3">
          <h4 className="text-sm font-semibold uppercase tracking-wider text-muted-foreground">
            Cadence and dwell
          </h4>
          <div className="grid grid-cols-1 md:grid-cols-2 gap-4">
            <div className="space-y-2">
              <div className="flex justify-between text-xs">
                <span>Cadence scale</span>
                <span>{cadenceScale.toFixed(2)}×</span>
              </div>
              <Slider
                value={[cadenceScale]}
                min={0.25}
                max={2}
                step={0.25}
                onValueChange={(value) => setCadenceScale(value[0])}
                disabled={recording}
              />
            </div>
            <div className="space-y-2">
              <div className="flex justify-between text-xs">
                <span>Settle dwell</span>
                <span>{settleDwellSec.toFixed(2)} s</span>
              </div>
              <Slider
                value={[settleDwellSec]}
                min={0}
                max={1}
                step={0.05}
                onValueChange={(value) => setSettleDwellSec(value[0])}
                disabled={recording}
              />
            </div>
          </div>
        </div>

        <div className="space-y-3">
          <h4 className="text-sm font-semibold uppercase tracking-wider text-muted-foreground">
            Landmarks
          </h4>
          {!hasDraft || landmarks.length === 0 ? (
            <p className="text-xs text-muted-foreground italic">Stop recording to extract full-q landmarks.</p>
          ) : (
            <ul className="space-y-2">
              {landmarks.map((landmark) => (
                <li key={landmark.id} className="flex items-start gap-3 rounded border border-border/50 bg-muted/20 p-2 text-xs">
                  <Checkbox
                    checked={landmark.included}
                    onCheckedChange={(checked) => setLandmarkIncluded(landmark.id, !!checked)}
                    id={landmark.id}
                  />
                  <label htmlFor={landmark.id} className="flex-1 cursor-pointer">
                    {landmark.label} at {landmark.tSec.toFixed(2)}s
                  </label>
                </li>
              ))}
            </ul>
          )}
        </div>
      </CardContent>
    </Card>
  );
}
