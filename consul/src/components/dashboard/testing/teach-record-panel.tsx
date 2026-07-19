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
import {
  canApplyLandmarks,
  extractLandmarks,
  samplesHaveMotion,
} from '@/lib/teach-record';
import { subscribeTeachSamples } from '@/lib/teach-sample-bus';
import {
  createTeachSession,
  liveFingerprint,
  sessionToWaveOverlay,
} from '@/lib/teach-transit';
import { useCompoundStore } from '@/state/compoundStore';
import { useHostMetricsStore } from '@/state/hostMetricsStore';
import { useRobotStore } from '@/state/robotStore';
import { useTeachStore } from '@/state/teachStore';

const WAVE_PRESET_ID = 'wave';

/**
 * Separate teach-record panel (not bolted into compound-test-panel).
 * Record requires GravityComp preflight checkbox + ACTIVE; posts no POSITION.
 */
export function TeachRecordPanel() {
  const operationalMode = useRobotStore((s) => s.operationalMode);
  const connected = useRobotStore((s) => s.connected);
  const compoundRunning = useCompoundStore((s) => s.isRunning);
  const returnHomePending = useCompoundStore((s) => s.returnHomePending);
  const { data: config = null } = useConfigSnapshot();
  const piMetrics = useHostMetricsStore((s) => s.piMetrics);

  const recording = useTeachStore((s) => s.recording);
  const gravityArmed = useTeachStore((s) => s.gravityArmed);
  const sampleCount = useTeachStore((s) => s.samples.length);
  const landmarks = useTeachStore((s) => s.landmarks);
  const cadenceScale = useTeachStore((s) => s.cadenceScale);
  const settleDwellSec = useTeachStore((s) => s.settleDwellSec);
  const lastError = useTeachStore((s) => s.lastError);
  const overlays = useTeachStore((s) => s.overlays);
  const liveCalibrationEpoch = useTeachStore((s) => s.liveCalibrationEpoch);
  const setGravityArmed = useTeachStore((s) => s.setGravityArmed);
  const setRecording = useTeachStore((s) => s.setRecording);
  const appendSample = useTeachStore((s) => s.appendSample);
  const clearSamples = useTeachStore((s) => s.clearSamples);
  const setLandmarks = useTeachStore((s) => s.setLandmarks);
  const setLandmarkIncluded = useTeachStore((s) => s.setLandmarkIncluded);
  const setCadenceScale = useTeachStore((s) => s.setCadenceScale);
  const setSettleDwellSec = useTeachStore((s) => s.setSettleDwellSec);
  const setLastError = useTeachStore((s) => s.setLastError);
  const applyOverlay = useTeachStore((s) => s.applyOverlay);
  const clearOverlay = useTeachStore((s) => s.clearOverlay);
  const resetSession = useTeachStore((s) => s.resetSession);
  const markCalibrationChanged = useTeachStore((s) => s.markCalibrationChanged);
  const acknowledgeCalibration = useTeachStore((s) => s.acknowledgeCalibration);

  const waveBase = COMPOUND_TEST_PRESETS.find((p) => p.id === WAVE_PRESET_ID);
  const joints = waveBase?.joints ?? [];
  const waveOverlay = overlays[WAVE_PRESET_ID];
  const hasOverlay = Boolean(waveOverlay);
  const needsCalAck = overlayNeedsCalibrationAck(waveOverlay?.session, {
    liveCalibrationEpoch,
    ackedAtEpoch: waveOverlay?.ackedAtEpoch ?? 0,
  });

  // Only subscribe while recording so unthrottled Chappe ticks do not allocate
  // teach samples (and zustand set) when the forceMounted Teach tab is idle.
  React.useEffect(() => {
    if (!recording) return;
    return subscribeTeachSamples((sample) => {
      appendSample(sample);
    });
  }, [recording, appendSample]);

  // Fail closed if operator leaves ACTIVE or telemetry drops (mode stays stale).
  React.useEffect(() => {
    if (!recording) return;
    if (operationalMode !== 'ACTIVE' || !connected) {
      setRecording(false);
      setLastError(
        !connected
          ? 'Record stopped — Chappe disconnected (safety mode unknown).'
          : 'Record stopped — robot left ACTIVE (GravityComp / enable required).'
      );
    }
  }, [recording, operationalMode, connected, setRecording, setLastError]);

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
          ? 'Wait for Wave return-home to settle before Record.'
          : 'Record requires: gravity-comp armed checkbox, ACTIVE, and Wave stopped. Run gravity-on on Pi first (Testing posts POSITION and will kick GravityComp).'
      );
      return;
    }
    clearSamples();
    setLandmarks([]);
    setLastError(null);
    setRecording(true);
  };

  const stopRecord = () => {
    setRecording(false);
    const buf = useTeachStore.getState().samples;
    if (!samplesHaveMotion(buf, joints)) {
      setLandmarks([]);
      setLastError('No motion in buffer — fail closed; nothing to Apply.');
      return;
    }
    const extracted = extractLandmarks(buf, joints);
    setLandmarks(extracted);
    if (!canApplyLandmarks(extracted)) {
      setLastError('Landmark extraction empty/failed — fail closed; do not Apply.');
    } else {
      setLastError(null);
    }
  };

  const onApply = () => {
    if (!waveBase) return;
    if (!canApplyLandmarks(landmarks)) {
      setLastError('Cannot Apply — need ≥2 included landmarks with full-q snapshots.');
      return;
    }
    const profile = config?.profile ?? 'arm_3dof_right';
    const fp = liveFingerprint(profile, joints, piMetrics?.build);
    const liveEpoch = useTeachStore.getState().liveCalibrationEpoch;
    const session = createTeachSession(fp, WAVE_PRESET_ID, landmarks, {
      cadenceScale,
      settleDwellSec,
      calibrationEpoch: liveEpoch,
    });
    const result = sessionToWaveOverlay(session, waveBase, fp);
    if (!result.ok) {
      setLastError(`Apply refused: ${result.error}`);
      return;
    }
    // Persist session only — Wave materializes keyframes at play time.
    applyOverlay(WAVE_PRESET_ID, {
      session,
      ackedAtEpoch: liveEpoch,
    });
    // Sync Loop checkbox only when Wave is idle — live interval reads store.loop.
    const compound = useCompoundStore.getState();
    if (!compound.isRunning) {
      compound.setLoop(result.preset.loop);
    }
  };

  const onDownload = () => {
    const entry = overlays[WAVE_PRESET_ID];
    const payload = entry ?? {
      landmarks,
      cadenceScale,
      settleDwellSec,
      samples: sampleCount,
    };
    const blob = new Blob([JSON.stringify(payload, null, 2)], {
      type: 'application/json',
    });
    const url = URL.createObjectURL(blob);
    const a = document.createElement('a');
    a.href = url;
    a.download = `teach-wave-${Date.now()}.json`;
    a.click();
    URL.revokeObjectURL(url);
  };

  return (
    <Card variant="panel" className={dashboardPanelCardClassName}>
      <CardHeader>
        <CardTitle className="text-lg">Teach Record</CardTitle>
        <CardDescription>
          Record a GravityComp wave (manual), extract full-q landmarks, set cadence/dwell,
          Apply overlay onto Wave. Keeps nativeWave until Apply. Mutually exclusive with
          Wave playback.
        </CardDescription>
      </CardHeader>
      <CardContent className="space-y-6">
        <div className="rounded-md border border-border/60 bg-muted/20 p-3 text-xs text-muted-foreground space-y-2">
          <p>
            Preflight: on Pi run <code className="text-foreground">gravity-on</code> (MCP
            or marengo-pi). Consul Testing cannot enter GravityComp — do not Start Wave
            while recording (POSITION posts end gravity-comp).
          </p>
          <div className="flex items-center gap-2">
            <Checkbox
              id="gravity-armed"
              checked={gravityArmed}
              onCheckedChange={(c) => setGravityArmed(!!c)}
              disabled={recording}
            />
            <label htmlFor="gravity-armed" className="cursor-pointer font-medium text-foreground">
              Gravity-comp armed (I ran gravity-on; ACTIVE)
            </label>
          </div>
          <div className="flex flex-wrap gap-2">
            <Badge variant="secondary">mode: {operationalMode ?? '—'}</Badge>
            <Badge variant="secondary">samples: {sampleCount}</Badge>
            <Badge variant="secondary">cal epoch: {liveCalibrationEpoch}</Badge>
            {hasOverlay ? <Badge>overlay on Wave</Badge> : null}
            {needsCalAck ? <Badge variant="destructive">cal ack needed</Badge> : null}
          </div>
        </div>

        {needsCalAck ? (
          <div className="rounded-md border border-amber-500/40 bg-amber-500/10 p-3 text-xs space-y-2">
            <p className="text-foreground font-medium">
              Calibration may have changed (set-zero) since this overlay was applied.
              Overlay kept — Wave falls back to shipped preset until you Acknowledge or Reset.
            </p>
            <p className="text-muted-foreground">
              Home alone does not trigger this. After MCP <code>pi_set_zero</code> or
              motor-repl set-zero on teach joints, click <strong>I set-zero&apos;d</strong>
              (Consul has no machine signal yet — agents must remind the operator).
            </p>
            <div className="flex flex-wrap gap-2">
              <Button size="sm" onClick={() => acknowledgeCalibration(WAVE_PRESET_ID)}>
                Acknowledge &amp; keep
              </Button>
              <Button
                size="sm"
                variant="outline"
                onClick={() => {
                  resetSession();
                  clearOverlay(WAVE_PRESET_ID);
                }}
              >
                Reset overlay
              </Button>
            </div>
          </div>
        ) : null}

        <div className="flex flex-wrap gap-2">
          {recording ? (
            <Button variant="destructive" onClick={stopRecord}>
              Stop Record
            </Button>
          ) : (
            <Button onClick={startRecord} disabled={!canRecord}>
              Record
            </Button>
          )}
          <Button
            variant="outline"
            onClick={onApply}
            disabled={recording || !canApplyLandmarks(landmarks)}
          >
            Apply to Wave
          </Button>
          <Button variant="outline" onClick={onDownload} disabled={landmarks.length === 0 && !hasOverlay}>
            Download
          </Button>
          <Button
            variant="secondary"
            onClick={() => markCalibrationChanged()}
            disabled={!hasOverlay || recording}
            title="After set-zero of teach joints — not after ordinary Home"
          >
            I set-zero&apos;d
          </Button>
          <Button
            variant="ghost"
            onClick={() => {
              resetSession();
              clearOverlay(WAVE_PRESET_ID);
            }}
          >
            Reset
          </Button>
        </div>
        {compoundRunning ? (
          <p className="text-xs text-muted-foreground">
            Wave playback is running — Stop Wave before Record.
          </p>
        ) : null}
        {returnHomePending ? (
          <p className="text-xs text-muted-foreground">
            Wave return-home in progress — wait before Record.
          </p>
        ) : null}

        {lastError ? (
          <p className="text-sm text-destructive" role="alert">
            {lastError}
          </p>
        ) : null}

        <div className="space-y-3">
          <h4 className="text-sm font-semibold uppercase tracking-wider text-muted-foreground">
            Cadence / dwell
          </h4>
          <p className="text-xs text-muted-foreground">
            Cadence scales when Consul posts the next endpoint (taught Δt × scale). Not
            Berthier transit speed. Runner speed multiplier still applies once at playback.
          </p>
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
                onValueChange={(v) => setCadenceScale(v[0])}
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
                onValueChange={(v) => setSettleDwellSec(v[0])}
                disabled={recording}
              />
            </div>
          </div>
        </div>

        <div className="space-y-3">
          <h4 className="text-sm font-semibold uppercase tracking-wider text-muted-foreground">
            Landmarks (full-q)
          </h4>
          {landmarks.length === 0 ? (
            <p className="text-xs text-muted-foreground italic">
              Stop Record to extract landmarks from the unthrottled sample buffer.
            </p>
          ) : (
            <ul className="space-y-2">
              {landmarks.map((lm) => (
                <li
                  key={lm.id}
                  className="flex items-start gap-3 rounded border border-border/50 bg-muted/20 p-2 text-xs"
                >
                  <Checkbox
                    checked={lm.included}
                    onCheckedChange={(c) => setLandmarkIncluded(lm.id, !!c)}
                    id={lm.id}
                  />
                  <label htmlFor={lm.id} className="flex-1 cursor-pointer space-y-1">
                    <div className="font-medium">
                      {lm.label} @ {lm.tSec.toFixed(2)}s
                    </div>
                    <div className="text-muted-foreground font-mono break-all">
                      {joints
                        .map((j) => `${j}=${(lm.q[j] ?? 0).toFixed(3)}`)
                        .join(' · ')}
                    </div>
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
