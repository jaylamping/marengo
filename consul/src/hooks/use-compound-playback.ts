import * as React from 'react';
import { create } from '@bufbuild/protobuf';
import {
  ControlMode,
  MitCommandBatchSchema,
  MitJointCommandSchema,
  type MitJointCommand,
} from '@/gen/marengo/v1/marengo_pb';
import {
  compoundPresetById,
  WAVE_POSE_GCOMP_SIGNED,
} from '@/data/compound-tests';
import {
  encodeNativeWaveCommandName,
  jointsSettled,
  nativeWaveDurationSec,
  presetSegmentCount,
  segmentDurationSec,
  segmentTargets,
  WAYPOINT_SETTLE_HOLD_SEC,
} from '@/lib/compound-runner';
import { postTestingMitCommandBatch } from '@/lib/gateway-api';
import { overlayNeedsCalibrationAck } from '@/lib/teach-calibration';
import { liveFingerprint, resolvePlayablePreset } from '@/lib/teach-transit';
import { useConfigSnapshot } from '@/hooks/use-config-snapshot';
import { useCompoundStore } from '@/state/compoundStore';
import { useHostMetricsStore } from '@/state/hostMetricsStore';
import { useRobotStore } from '@/state/robotStore';
import { useTeachStore } from '@/state/teachStore';
import { useTestingStore } from '@/state/testingStore';

const CONFIG_GAINS = { kp: 0, kd: 0, ki: 0, fc: 0 };

interface CompoundRunner {
  intervalId: number | null;
  segmentIndex: number;
  segmentStartedAtMs: number;
  activeTargets: Record<string, number>;
  posting: boolean;
  settledSinceMs: number | null;
  phase: 'raise' | 'wave';
  waveEndsAtMs: number | null;
}

const initialRunner: CompoundRunner = {
  intervalId: null,
  segmentIndex: 0,
  segmentStartedAtMs: 0,
  activeTargets: {},
  posting: false,
  settledSinceMs: null,
  phase: 'raise',
  waveEndsAtMs: null,
};

export function useCompoundPlayback() {
  const selectedPresetId = useCompoundStore((state) => state.selectedPresetId);
  const dryRun = useTestingStore((state) => state.dryRun);
  const operationalMode = useRobotStore((state) => state.operationalMode);
  const connected = useRobotStore((state) => state.connected);
  const isRunning = useCompoundStore((state) => state.isRunning);
  const overlays = useTeachStore((state) => state.overlays);
  const liveCalibrationEpoch = useTeachStore(
    (state) => state.liveCalibrationEpoch,
  );
  const { data: config = null } = useConfigSnapshot();
  const [overlayBlockReason, setOverlayBlockReason] = React.useState<
    string | null
  >(null);
  const usingOverlayRef = React.useRef(false);
  const runnerRef = React.useRef<CompoundRunner>(initialRunner);

  React.useEffect(() => {
    setOverlayBlockReason(null);
  }, [selectedPresetId]);

  const postPositionBatch = React.useCallback(
    async (joints: MitJointCommand[]) => {
      useTeachStore.getState().setGravityArmed(false);
      await postTestingMitCommandBatch(
        create(MitCommandBatchSchema, {
          timestampMs: BigInt(Date.now()),
          mode: ControlMode.POSITION,
          joints,
        }),
      );
    },
    [],
  );

  const returnHome = React.useCallback(
    async (jointNames: string[]) => {
      const joints = jointNames.map((name) =>
        create(MitJointCommandSchema, {
          name,
          ...CONFIG_GAINS,
          position: 0,
          velocity: 0,
          torqueFf: 0,
        }),
      );
      if (!useTestingStore.getState().dryRun && joints.length > 0) {
        await postPositionBatch(joints);
      }
    },
    [postPositionBatch],
  );

  const stopRunner = React.useCallback(
    (opts?: { returnHome?: boolean }) => {
      if (runnerRef.current.intervalId !== null) {
        window.clearInterval(runnerRef.current.intervalId);
        runnerRef.current.intervalId = null;
      }
      const compound = useCompoundStore.getState();
      compound.setIsRunning(false);
      compound.setProgress(0);
      if (opts?.returnHome === false || !selectedPresetId) return;
      const base = compoundPresetById(selectedPresetId);
      if (!base) return;

      const teach = useTeachStore.getState();
      const profile = config?.profile ?? 'arm_4dof_right';
      const liveFp = liveFingerprint(
        profile,
        base.joints,
        useHostMetricsStore.getState().piMetrics?.build,
      );
      const playable = resolvePlayablePreset(
        base,
        teach.overlays[selectedPresetId],
        liveFp,
        teach.liveCalibrationEpoch,
      );
      compound.setReturnHomePending(true);
      void (async () => {
        try {
          await returnHome(playable.preset.joints);
          await new Promise((resolve) => window.setTimeout(resolve, 2500));
        } finally {
          useCompoundStore.getState().setReturnHomePending(false);
        }
      })();
    },
    [config?.profile, returnHome, selectedPresetId],
  );

  React.useEffect(
    () => () => {
      if (runnerRef.current.intervalId !== null) {
        window.clearInterval(runnerRef.current.intervalId);
        runnerRef.current.intervalId = null;
      }
      useCompoundStore.getState().setIsRunning(false);
    },
    [],
  );

  React.useEffect(() => {
    if (selectedPresetId !== null) return;
    if (runnerRef.current.intervalId !== null) {
      window.clearInterval(runnerRef.current.intervalId);
      runnerRef.current.intervalId = null;
    }
    const compound = useCompoundStore.getState();
    compound.setIsRunning(false);
    compound.setProgress(0);
  }, [selectedPresetId]);

  React.useEffect(() => {
    if (isRunning && !dryRun && (operationalMode !== 'ACTIVE' || !connected)) {
      stopRunner({ returnHome: false });
    }
  }, [connected, dryRun, isRunning, operationalMode, stopRunner]);

  React.useEffect(() => {
    if (!isRunning || !selectedPresetId || !usingOverlayRef.current) return;
    const entry = overlays[selectedPresetId];
    if (
      entry &&
      overlayNeedsCalibrationAck(entry.session, {
        liveCalibrationEpoch,
        ackedAtEpoch: entry.ackedAtEpoch,
      })
    ) {
      stopRunner({ returnHome: true });
      setOverlayBlockReason(
        'Taught overlay needs Acknowledge & keep (or Reset). The shipped preset is available after Stop settles.',
      );
    }
  }, [isRunning, liveCalibrationEpoch, overlays, selectedPresetId, stopRunner]);

  const startRunner = React.useCallback(() => {
    if (
      !selectedPresetId ||
      useTeachStore.getState().capture.kind === 'recording'
    )
      return;
    if (useCompoundStore.getState().returnHomePending) {
      setOverlayBlockReason(
        'Wait for return-home to settle before starting Wave.',
      );
      return;
    }
    // Read live — Test proposal sets Dry Run then starts in the same tick.
    const dryRunNow = useTestingStore.getState().dryRun;
    if (!dryRunNow && (operationalMode !== 'ACTIVE' || !connected)) {
      setOverlayBlockReason(
        !connected
          ? 'Chappe disconnected — cannot start Wave until telemetry reconnects (or enable Dry Run).'
          : 'Robot must be ACTIVE to start Wave (or enable Dry Run).',
      );
      return;
    }

    const base = compoundPresetById(selectedPresetId);
    if (!base) return;
    if (!dryRunNow && base.id === 'wave' && !WAVE_POSE_GCOMP_SIGNED) {
      setOverlayBlockReason(
        'Live Wave is blocked until E6 Wave-pose GravityComp is signed (docs/bench-elbow-test-suite.md). Use Dry Run, or flip WAVE_POSE_GCOMP_SIGNED after sign-off.',
      );
      return;
    }

    const teach = useTeachStore.getState();
    const liveFp = liveFingerprint(
      config?.profile ?? 'arm_4dof_right',
      base.joints,
      useHostMetricsStore.getState().piMetrics?.build,
    );
    const playable = resolvePlayablePreset(
      base,
      teach.overlays[selectedPresetId],
      liveFp,
      teach.liveCalibrationEpoch,
    );
    const preset = playable.preset;
    usingOverlayRef.current = playable.usingOverlay;
    setOverlayBlockReason(playable.warning);

    const segmentCount = presetSegmentCount(preset.keyframes);
    if (segmentCount === 0) return;
    const limits = Object.fromEntries(
      preset.joints.map((joint) => {
        const motor = config?.motors.find((entry) => entry.joint === joint);
        return [
          joint,
          {
            min: motor?.bench.position_lower_rad ?? -Math.PI,
            max: motor?.bench.position_upper_rad ?? Math.PI,
          },
        ];
      }),
    );

    const postSegment = async (
      segmentIndex: number,
      trims: Record<string, number>,
      dry: boolean,
    ) => {
      const targets = segmentTargets(
        preset.joints,
        preset.keyframes,
        segmentIndex,
        trims,
        limits,
      );
      Object.assign(runnerRef.current, {
        activeTargets: targets,
        segmentIndex,
        segmentStartedAtMs: Date.now(),
        settledSinceMs: null,
      });
      const commands = Object.entries(targets).map(([name, position]) =>
        create(MitJointCommandSchema, {
          name,
          ...CONFIG_GAINS,
          position,
          velocity: 0,
          torqueFf: 0,
        }),
      );
      if (!dry && commands.length > 0) await postPositionBatch(commands);
    };

    const postNativeWave = async (dry: boolean, speedMultiplier: number) => {
      const wave = preset.nativeWave;
      if (!wave) return;
      const halfPeriodSec = Math.max(
        0.05,
        wave.halfPeriodSec / speedMultiplier,
      );
      const durationSec = nativeWaveDurationSec({ ...wave, halfPeriodSec });
      Object.assign(runnerRef.current, {
        phase: 'wave',
        segmentStartedAtMs: Date.now(),
        waveEndsAtMs: Date.now() + durationSec * 1000,
        activeTargets: {
          ...runnerRef.current.activeTargets,
          [wave.joint]: wave.maxRad,
        },
      });
      const command = create(MitJointCommandSchema, {
        name: encodeNativeWaveCommandName(wave, speedMultiplier),
        ...CONFIG_GAINS,
        position: wave.minRad,
        velocity: 0,
        torqueFf: 0,
      });
      if (!dry) await postPositionBatch([command]);
    };

    Object.assign(runnerRef.current, {
      segmentIndex: 0,
      activeTargets: {},
      posting: false,
      phase: 'raise',
      waveEndsAtMs: null,
    });
    useCompoundStore.getState().setIsRunning(true);
    void postSegment(
      0,
      useCompoundStore.getState().trims,
      useTestingStore.getState().dryRun,
    );

    runnerRef.current.intervalId = window.setInterval(() => {
      if (runnerRef.current.posting) return;
      const compound = useCompoundStore.getState();
      const nowMs = Date.now();
      const speedMultiplier = Math.max(0.25, compound.speedMultiplier);
      const robot = useRobotStore.getState().robotState;
      const measured: Record<string, number | undefined> = {};
      const measuredVelocity: Record<string, number | undefined> = {};
      for (const joint of preset.joints) {
        const state = robot?.joints.find((entry) => entry.name === joint);
        measured[joint] = state?.position;
        measuredVelocity[joint] = state?.velocity;
      }

      if (
        runnerRef.current.phase === 'wave' &&
        runnerRef.current.waveEndsAtMs !== null
      ) {
        const durationMs = Math.max(
          1,
          runnerRef.current.waveEndsAtMs - runnerRef.current.segmentStartedAtMs,
        );
        const progress =
          0.2 +
          0.8 * ((nowMs - runnerRef.current.segmentStartedAtMs) / durationMs);
        compound.setProgress(Math.min(progress, 1));
        if (nowMs < runnerRef.current.waveEndsAtMs) return;
        if (compound.loop && preset.nativeWave) {
          const halfPeriodSec = Math.max(
            0.05,
            preset.nativeWave.halfPeriodSec / speedMultiplier,
          );
          const durationSec = nativeWaveDurationSec({
            ...preset.nativeWave,
            halfPeriodSec,
          });
          runnerRef.current.segmentStartedAtMs = nowMs;
          runnerRef.current.waveEndsAtMs = nowMs + durationSec * 1000;
          return;
        }
        stopRunner({ returnHome: true });
        return;
      }

      const segmentIndex = runnerRef.current.segmentIndex;
      const dwellSec =
        segmentDurationSec(preset.keyframes, segmentIndex) / speedMultiplier;
      const elapsedSec = (nowMs - runnerRef.current.segmentStartedAtMs) / 1000;
      const positionVelocityOk = jointsSettled(
        measured,
        runnerRef.current.activeTargets,
        undefined,
        measuredVelocity,
      );
      runnerRef.current.settledSinceMs = positionVelocityOk
        ? (runnerRef.current.settledSinceMs ?? nowMs)
        : null;
      const settledForSec =
        runnerRef.current.settledSinceMs === null
          ? 0
          : (nowMs - runnerRef.current.settledSinceMs) / 1000;
      const mayAdvance =
        (preset.advance ?? 'settle') === 'timed'
          ? elapsedSec >= dwellSec
          : elapsedSec >= dwellSec &&
            positionVelocityOk &&
            settledForSec >= WAYPOINT_SETTLE_HOLD_SEC;
      compound.setProgress(
        Math.min(
          (segmentIndex +
            (mayAdvance ? 1 : elapsedSec / Math.max(dwellSec, 1e-6))) /
            Math.max(segmentCount + (preset.nativeWave ? 1 : 0), 1),
          1,
        ),
      );
      if (!mayAdvance) return;

      let nextSegment = segmentIndex + 1;
      if (nextSegment >= segmentCount) {
        if (preset.nativeWave) {
          runnerRef.current.posting = true;
          void postNativeWave(
            useTestingStore.getState().dryRun,
            speedMultiplier,
          ).finally(() => {
            runnerRef.current.posting = false;
          });
          return;
        }
        if (!compound.loop || !preset.loop) {
          stopRunner({ returnHome: true });
          return;
        }
        nextSegment = preset.loopFromSegment ?? 0;
      }
      runnerRef.current.posting = true;
      void postSegment(
        nextSegment,
        compound.trims,
        useTestingStore.getState().dryRun,
      ).finally(() => {
        runnerRef.current.posting = false;
      });
    }, 100);
  }, [
    config,
    connected,
    dryRun,
    operationalMode,
    postPositionBatch,
    selectedPresetId,
    stopRunner,
  ]);

  return { overlayBlockReason, setOverlayBlockReason, startRunner, stopRunner };
}
