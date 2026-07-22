import * as React from 'react';
import {
  assertAutoLearnResponse,
  nextStage,
  stageEnvelope,
  type AutoLearnLandmark,
  type AutoLearnStage,
} from '@marengo/compound-auto-learn';
import {
  compoundPresetById,
  WAVE_POSE_GCOMP_SIGNED,
} from '@/data/compound-tests';
import { useConfigSnapshot } from '@/hooks/use-config-snapshot';
import { autoLearnConfigured, postAutoLearn } from '@/lib/auto-learn-api';
import { buildAutoLearnLogContext } from '@/lib/auto-learn-logs';
import { buildAutoLearnRequest } from '@/lib/auto-learn-snapshot';
import { canApplyLandmarks, type TeachLandmark } from '@/lib/teach-record';
import {
  createTeachSession,
  liveFingerprint,
  materializeTaughtPreset,
} from '@/lib/teach-transit';
import { useAutoLearnStore } from '@/state/autoLearnStore';
import { useCompoundStore } from '@/state/compoundStore';
import { useHostMetricsStore } from '@/state/hostMetricsStore';
import { useRobotStore } from '@/state/robotStore';
import { useTeachStore } from '@/state/teachStore';
import { useTestingStore } from '@/state/testingStore';

function setAutoLearnError(message: string): void {
  useAutoLearnStore.getState().setStatus('error', message);
}

function assertFailuresMessage(
  failures: { message: string }[],
): string {
  return failures.map((f) => f.message).join('; ');
}

export function useAutoLearnController(presetId: string) {
  const base = compoundPresetById(presetId);
  const { data: config = null } = useConfigSnapshot();
  const robotState = useRobotStore((s) => s.robotState);
  const connected = useRobotStore((s) => s.connected);
  const operationalMode = useRobotStore((s) => s.operationalMode);
  const piMetrics = useHostMetricsStore((s) => s.piMetrics);
  const compoundRunning = useCompoundStore((s) => s.isRunning);
  const capture = useTeachStore((s) => s.capture);
  const overlays = useTeachStore((s) => s.overlays);

  const stage = useAutoLearnStore((s) => s.stage);
  const includeLogs = useAutoLearnStore((s) => s.includeLogs);
  const feedback = useAutoLearnStore((s) => s.feedback);
  const status = useAutoLearnStore((s) => s.status);
  const error = useAutoLearnStore((s) => s.error);
  const logAttachNote = useAutoLearnStore((s) => s.logAttachNote);
  const proposalTested = useAutoLearnStore((s) => s.proposalTested);
  const reviewHint = useAutoLearnStore((s) => s.reviewHint);
  const draft = useAutoLearnStore((s) => s.draft);
  const draftForPreset = draft?.presetId === presetId ? draft : null;

  const abortRef = React.useRef<AbortController | null>(null);
  const configured = autoLearnConfigured();
  const recording = capture.kind === 'recording';

  const priorLandmarks = React.useMemo((): AutoLearnLandmark[] | null => {
    if (draftForPreset) return draftForPreset.landmarks;
    const session = overlays[presetId]?.session;
    if (!session?.landmarks?.length) return null;
    return session.landmarks;
  }, [draftForPreset, overlays, presetId]);

  const priorDescription = draftForPreset
    ? draftForPreset.description
    : overlays[presetId]?.session
      ? 'Applied teach overlay'
      : null;

  const hasPrior = priorLandmarks != null;

  React.useEffect(() => {
    if (!hasPrior && stage !== 'crawl') {
      useAutoLearnStore.getState().setStage('crawl');
    }
  }, [hasPrior, stage]);

  const resolveLogContext = async (forceWithoutLogs: boolean) => {
    const store = useAutoLearnStore.getState();
    if (!store.includeLogs) {
      store.setLogAttachNote(null);
      return { ok: true as const, context: null };
    }
    if (forceWithoutLogs) {
      store.setLogAttachNote('continuing without logs');
      return { ok: true as const, context: null };
    }
    const result = await buildAutoLearnLogContext(store.lastGenerateAtMs);
    if (!result.ok) {
      store.setLogAttachNote(result.message);
      return { ok: false as const, message: result.message };
    }
    store.setLogAttachNote(
      result.context.truncated
        ? 'logs attached (truncated)'
        : 'logs attached',
    );
    return { ok: true as const, context: result.context };
  };

  const runGenerate = async (opts: {
    stage: AutoLearnStage;
    usePrior: boolean;
    continueWithoutLogs?: boolean;
  }) => {
    if (!base) return;
    if (recording || compoundRunning) {
      setAutoLearnError('Stop recording / compound playback first');
      return;
    }
    if (!configured) {
      setAutoLearnError(
        'Set Auto Learn operator token (Vite VITE_AUTO_LEARN_OPERATOR_TOKEN or paste in panel) and ensure gateway is reachable',
      );
      return;
    }

    const logs = await resolveLogContext(Boolean(opts.continueWithoutLogs));
    if (!logs.ok) {
      setAutoLearnError(`LOGS_CONFIRM:${logs.message}`);
      return;
    }

    const built = buildAutoLearnRequest({
      preset: base,
      stage: opts.stage,
      operatorFeedback: useAutoLearnStore.getState().feedback,
      config,
      robotJoints: robotState?.joints ?? [],
      priorLandmarks: opts.usePrior ? priorLandmarks : null,
      priorDescription: opts.usePrior ? priorDescription : null,
      logContext: logs.context,
    });
    if (!built.ok) {
      setAutoLearnError(built.message);
      return;
    }

    const gen = useAutoLearnStore.getState().bumpGeneration();
    abortRef.current?.abort();
    const ac = new AbortController();
    abortRef.current = ac;
    useAutoLearnStore.getState().setStatus('calling');

    const result = await postAutoLearn(built.request, ac.signal);
    if (gen !== useAutoLearnStore.getState().requestGeneration) return;

    if (!result.ok) {
      setAutoLearnError(result.error.message);
      return;
    }

    const asserted = assertAutoLearnResponse(built.request, result.response);
    if (!asserted.ok) {
      setAutoLearnError(assertFailuresMessage(asserted.failures));
      return;
    }

    const env = stageEnvelope(result.response.stage);
    useCompoundStore
      .getState()
      .setSpeedMultiplier(
        Math.min(result.response.speedMultiplier, env.maxSpeedMultiplier),
      );
    useTeachStore.getState().setCadenceScale(result.response.cadenceScale);
    useTeachStore.getState().setSettleDwellSec(result.response.settleDwellSec);

    useAutoLearnStore.getState().setDraft({
      presetId,
      stage: result.response.stage,
      description: result.response.description,
      landmarks: result.response.landmarks,
      cadenceScale: result.response.cadenceScale,
      settleDwellSec: result.response.settleDwellSec,
      speedMultiplier: result.response.speedMultiplier,
    });
    useAutoLearnStore.getState().setStage(result.response.stage);
    useAutoLearnStore.getState().setStatus('draft');
  };

  const autoLearn = (continueWithoutLogs?: boolean) =>
    runGenerate({
      stage,
      usePrior: hasPrior,
      continueWithoutLogs,
    });

  const advanceStage = (continueWithoutLogs?: boolean) => {
    const nxt = nextStage(stage);
    if (!nxt || !hasPrior) return;
    useAutoLearnStore.getState().setStage(nxt);
    return runGenerate({
      stage: nxt,
      usePrior: true,
      continueWithoutLogs,
    });
  };

  /** Load draft into teach overlay so Playback can resolve it. */
  const commitDraftOverlay = (): boolean => {
    const store = useTeachStore.getState();
    const draftState = useAutoLearnStore.getState().draft;
    if (!base || !draftState || draftState.presetId !== presetId) {
      setAutoLearnError('No Auto Learn draft');
      return false;
    }
    const landmarks = draftState.landmarks as TeachLandmark[];
    if (!canApplyLandmarks(landmarks)) {
      setAutoLearnError('Need enough included landmarks to apply');
      return false;
    }
    const fingerprint = liveFingerprint(
      config?.profile ?? 'arm_4dof_right',
      base.joints,
      piMetrics?.build,
    );
    if (fingerprint.deployRev === 'unknown') {
      setAutoLearnError(
        'Fingerprint unknown — wait for Pi host metrics / deployRev',
      );
      return false;
    }

    const built = buildAutoLearnRequest({
      preset: base,
      stage: draftState.stage,
      operatorFeedback: null,
      config,
      robotJoints: robotState?.joints ?? [],
      priorLandmarks: draftState.stage === 'crawl' ? null : priorLandmarks,
      priorDescription: null,
      logContext: null,
    });
    if (!built.ok) {
      setAutoLearnError(built.message);
      return false;
    }
    const asserted = assertAutoLearnResponse(built.request, {
      stage: draftState.stage,
      description: draftState.description,
      landmarks: draftState.landmarks,
      cadenceScale: draftState.cadenceScale,
      settleDwellSec: draftState.settleDwellSec,
      speedMultiplier: draftState.speedMultiplier,
      source: 'auto_learn',
    });
    if (!asserted.ok) {
      setAutoLearnError(assertFailuresMessage(asserted.failures));
      return false;
    }

    const epoch = store.liveCalibrationEpoch;
    const session = createTeachSession(fingerprint, presetId, landmarks, {
      cadenceScale: draftState.cadenceScale,
      settleDwellSec: draftState.settleDwellSec,
      calibrationEpoch: epoch,
    });
    const result = materializeTaughtPreset(session, base, fingerprint);
    if (!result.ok) {
      setAutoLearnError(`Apply refused: ${result.error}`);
      return false;
    }
    if (!store.applyOverlay(presetId, { session, ackedAtEpoch: epoch })) {
      setAutoLearnError(store.lastError ?? 'Apply failed');
      return false;
    }
    const compound = useCompoundStore.getState();
    if (!compound.isRunning) compound.setLoop(result.preset.loop);
    const env = stageEnvelope(draftState.stage);
    compound.setSpeedMultiplier(
      Math.min(draftState.speedMultiplier, env.maxSpeedMultiplier),
    );
    store.setCadenceScale(draftState.cadenceScale);
    store.setSettleDwellSec(draftState.settleDwellSec);
    return true;
  };

  const testProposal = (startRunner: () => void) => {
    if (recording || compoundRunning) {
      setAutoLearnError('Stop recording / compound playback first');
      return;
    }
    if (!commitDraftOverlay()) return;
    useTestingStore.getState().setDryRun(true);
    useAutoLearnStore.getState().markProposalTested('dry_run');
    useAutoLearnStore.getState().setStatus('draft');
    startRunner();
  };

  const hardwareBlockReason = ((): string | null => {
    if (!connected) {
      return 'Live test needs Chappe connected.';
    }
    if (operationalMode !== 'ACTIVE') {
      return `Live test needs motors ACTIVE (Enable). Current: ${operationalMode ?? 'unknown'}.`;
    }
    if (base?.id === 'wave' && !WAVE_POSE_GCOMP_SIGNED) {
      return 'Live Wave blocked until E6 Wave-pose GravityComp is signed (WAVE_POSE_GCOMP_SIGNED). Use Dry Run Test proposal until then.';
    }
    return null;
  })();

  const testOnHardware = (startRunner: () => void) => {
    if (recording || compoundRunning) {
      setAutoLearnError('Stop recording / compound playback first');
      return;
    }
    if (hardwareBlockReason) {
      setAutoLearnError(hardwareBlockReason);
      return;
    }
    if (!commitDraftOverlay()) return;
    useTestingStore.getState().setDryRun(false);
    useAutoLearnStore.getState().markProposalTested('hardware');
    useAutoLearnStore.getState().setStatus('draft');
    startRunner();
  };

  const applyOverlay = () => {
    const draftState = useAutoLearnStore.getState().draft;
    if (!commitDraftOverlay() || !draftState) return;
    useAutoLearnStore.getState().markApplied(presetId, draftState.stage);
  };

  const discard = () => {
    useAutoLearnStore.getState().setDraft(null);
    useAutoLearnStore.getState().setStatus('idle');
  };

  return {
    configured,
    movementBrief: base?.movementBrief ?? null,
    stage,
    includeLogs,
    feedback,
    status,
    error,
    logAttachNote,
    reviewHint,
    draft: draftForPreset,
    hasPrior,
    proposalTested,
    recording,
    compoundRunning,
    hardwareBlockReason,
    setStage: (s: AutoLearnStage) => useAutoLearnStore.getState().setStage(s),
    setIncludeLogs: (v: boolean) =>
      useAutoLearnStore.getState().setIncludeLogs(v),
    setFeedback: (v: string) => useAutoLearnStore.getState().setFeedback(v),
    clearFeedback: () => useAutoLearnStore.getState().clearFeedback(),
    setLandmarkIncluded: (id: string, included: boolean) =>
      useAutoLearnStore.getState().setLandmarkIncluded(id, included),
    autoLearn,
    advanceStage,
    testProposal,
    testOnHardware,
    applyOverlay,
    discard,
  };
}
