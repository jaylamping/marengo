import * as React from 'react';
import { compoundPresetById } from '@/data/compound-tests';
import { useConfigSnapshot } from '@/hooks/use-config-snapshot';
import { overlayNeedsCalibrationAck } from '@/lib/teach-calibration';
import { canApplyLandmarks } from '@/lib/teach-record';
import { subscribeTeachSamples } from '@/lib/teach-sample-bus';
import {
  createTeachSession,
  liveFingerprint,
  materializeTaughtPreset,
} from '@/lib/teach-transit';
import { useCompoundStore } from '@/state/compoundStore';
import { useHostMetricsStore } from '@/state/hostMetricsStore';
import { useRobotStore } from '@/state/robotStore';
import {
  captureIsRecording,
  captureLandmarks,
  capturePresetId,
  captureSamples,
  useTeachStore,
} from '@/state/teachStore';

export function useRecordMovementController(presetId: string) {
  const base = compoundPresetById(presetId);
  const operationalMode = useRobotStore((state) => state.operationalMode);
  const connected = useRobotStore((state) => state.connected);
  const compoundRunning = useCompoundStore((state) => state.isRunning);
  const returnHomePending = useCompoundStore(
    (state) => state.returnHomePending,
  );
  const { data: config = null } = useConfigSnapshot();
  const piMetrics = useHostMetricsStore((state) => state.piMetrics);
  const capture = useTeachStore((state) => state.capture);
  const gravityArmed = useTeachStore((state) => state.gravityArmed);
  const cadenceScale = useTeachStore((state) => state.cadenceScale);
  const settleDwellSec = useTeachStore((state) => state.settleDwellSec);
  const lastError = useTeachStore((state) => state.lastError);
  const overlays = useTeachStore((state) => state.overlays);
  const liveCalibrationEpoch = useTeachStore(
    (state) => state.liveCalibrationEpoch,
  );
  const recording = captureIsRecording(capture);
  const captureId = capturePresetId(capture);
  const samples = captureSamples(capture);
  const landmarks = captureLandmarks(capture);
  const isRecordingThisPreset = recording && captureId === presetId;
  const hasDraft = capture.kind === 'draft' && captureId === presetId;
  const overlay = overlays[presetId];
  const joints = base?.joints ?? [];
  const needsCalAck = overlayNeedsCalibrationAck(overlay?.session, {
    liveCalibrationEpoch,
    ackedAtEpoch: overlay?.ackedAtEpoch ?? 0,
  });

  React.useEffect(() => {
    if (!recording || !captureId || captureId === presetId) return;
    useTeachStore
      .getState()
      .finishRecording(compoundPresetById(captureId)?.joints ?? []);
  }, [captureId, presetId, recording]);

  React.useEffect(() => {
    if (!isRecordingThisPreset) return;
    return subscribeTeachSamples(useTeachStore.getState().appendSample);
  }, [isRecordingThisPreset]);

  React.useEffect(() => {
    if (!isRecordingThisPreset || (operationalMode === 'ACTIVE' && connected))
      return;
    const store = useTeachStore.getState();
    store.finishRecording(joints);
    store.setLastError(
      !connected
        ? 'Record stopped because Chappe disconnected.'
        : 'Record stopped because the robot left ACTIVE.',
    );
  }, [connected, isRecordingThisPreset, joints, operationalMode]);

  const canRecord =
    gravityArmed &&
    connected &&
    operationalMode === 'ACTIVE' &&
    !compoundRunning &&
    !returnHomePending &&
    joints.length > 0;

  const startRecord = () => {
    const store = useTeachStore.getState();
    if (!canRecord) {
      store.setLastError(
        returnHomePending
          ? 'Wait for return-home to settle before recording movement.'
          : 'Recording requires the gravity-comp armed checkbox, ACTIVE, and a stopped compound test.',
      );
      return;
    }
    store.startRecording(presetId);
  };

  const stopRecord = () => useTeachStore.getState().finishRecording(joints);

  const apply = () => {
    const store = useTeachStore.getState();
    if (!base || !hasDraft || !canApplyLandmarks(landmarks)) {
      store.setLastError(
        'Cannot apply. Record movement for this preset first.',
      );
      return;
    }
    const fingerprint = liveFingerprint(
      config?.profile ?? 'arm_4dof_right',
      joints,
      piMetrics?.build,
    );
    const epoch = store.liveCalibrationEpoch;
    const session = createTeachSession(fingerprint, presetId, landmarks, {
      cadenceScale,
      settleDwellSec,
      calibrationEpoch: epoch,
    });
    const result = materializeTaughtPreset(session, base, fingerprint);
    if (!result.ok) {
      store.setLastError(`Apply refused: ${result.error}`);
      return;
    }
    if (!store.applyOverlay(presetId, { session, ackedAtEpoch: epoch })) return;
    const compound = useCompoundStore.getState();
    if (!compound.isRunning) compound.setLoop(result.preset.loop);
  };

  const download = () => {
    const payload = overlay ?? {
      landmarks,
      cadenceScale,
      settleDwellSec,
      samples: samples.length,
    };
    const url = URL.createObjectURL(
      new Blob([JSON.stringify(payload, null, 2)], {
        type: 'application/json',
      }),
    );
    const link = document.createElement('a');
    link.href = url;
    link.download = `teach-${presetId}-${Date.now()}.json`;
    link.click();
    URL.revokeObjectURL(url);
  };

  const reset = () => {
    const store = useTeachStore.getState();
    store.cancelRecording();
    store.clearOverlay(presetId);
  };

  return {
    base,
    joints,
    operationalMode,
    compoundRunning,
    returnHomePending,
    recording,
    isRecordingThisPreset,
    hasDraft,
    overlay,
    needsCalAck,
    gravityArmed,
    samples,
    landmarks,
    cadenceScale,
    settleDwellSec,
    lastError,
    canRecord,
    startRecord,
    stopRecord,
    apply,
    download,
    reset,
  };
}
