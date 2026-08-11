import { useEffect, useRef, useState } from 'react';
import { toast } from 'sonner';

import {
  phaseLabel,
  statusCaption,
  type SidebarUpdateUiMode,
} from '@/components/dashboard/sidebar/sidebar-update-status-view';
import {
  SELF_UPDATE_TIMEOUT_MS,
  clearSelfUpdateSession,
  fetchVersionStatus,
  readSelfUpdateSession,
  shasMatch,
  shortSha,
  startSelfDeploy,
  writeSelfUpdateSession,
  type SelfUpdateSession,
  type UpdateUiState,
  type VersionStatusDto,
} from '@/lib/version-api';

const IDLE_POLL_MS = 60_000;
const BUSY_POLL_MS = 2_500;
/** Cap hung `/version/status` fetches so a gateway restart cannot freeze the poll loop. */
export const POLL_FETCH_TIMEOUT_MS = 8_000;

const UI_STATES: ReadonlySet<string> = new Set([
  'unknown',
  'current',
  'stale',
  'upstream_unknown',
  'updating',
  'failed',
]);

type DeriveSidebarUpdateModeOpts = {
  deployBusy?: boolean;
  /** sessionStorage bookmark from this tab's Update click; survives reload. */
  watchingJob?: boolean;
  /**
   * After a watched job fails we clear the session (idle poll) but keep Failed
   * chrome so gateway Failed→Stale does not flash "behind".
   */
  stickyFailed?: boolean;
};

/** True when installed deploy-rev already matches the job/session target. */
export function installedOnWatchTarget(
  status: VersionStatusDto,
  session?: Pick<SelfUpdateSession, 'targetSha'> | null,
): boolean {
  const targets = [session?.targetSha, status.deploy.target_sha].filter(
    (value): value is string => Boolean(value && value.trim()),
  );
  return targets.some((target) => shasMatch(status.deploy_sha, target));
}

export type WatchOutcome = 'success' | 'failed' | null;

/**
 * Decide whether a watched self-update can leave the Updating chrome.
 * Success includes “deploy-rev already on target” so a stale Queued/running
 * ledger after install cannot pin the sidebar forever.
 */
export function resolveWatchOutcome(
  status: VersionStatusDto,
  session: SelfUpdateSession,
): WatchOutcome {
  const sameJob = !status.deploy.job_id || status.deploy.job_id === session.jobId;
  if (sameJob && (status.deploy.state === 'failed' || status.ui_state === 'failed')) {
    return 'failed';
  }
  if (
    sameJob &&
    status.deploy.state === 'succeeded' &&
    status.ready_for_target
  ) {
    return 'success';
  }
  if (installedOnWatchTarget(status, session)) {
    return 'success';
  }
  return null;
}

/** Prefer gateway `ui_state`; fall back only for older gateways mid-rollout. */
export function deriveSidebarUpdateMode(
  status: VersionStatusDto | null,
  opts: DeriveSidebarUpdateModeOpts = {},
): SidebarUpdateUiMode {
  const deployBusy = opts.deployBusy ?? false;
  const watchingJob = opts.watchingJob ?? false;
  const stickyFailed = opts.stickyFailed ?? false;

  if (deployBusy) return 'updating';

  if (watchingJob) {
    if (!status) return 'updating';
    const jobState = status.deploy.state;
    // Gateway maps Failed+behind → Stale for retry chrome; keep Failed while watching.
    if (jobState === 'failed' || status.ui_state === 'failed') {
      return 'failed';
    }
    // Install already landed — do not keep “Updating · Queued” on a stale ledger.
    if (installedOnWatchTarget(status)) {
      if (status.ui_state && UI_STATES.has(status.ui_state) && status.ui_state !== 'updating') {
        return status.ui_state;
      }
      return status.update_available ? 'stale' : 'current';
    }
    if (jobState === 'running' || status.ui_state === 'updating') {
      return 'updating';
    }
    if (jobState !== 'succeeded') {
      return 'updating';
    }
  }

  // Session cleared on failure for idle polling — sticky keeps Failed (not "behind").
  if (stickyFailed) {
    return 'failed';
  }

  if (!status) return 'unknown';
  if (status.ui_state && UI_STATES.has(status.ui_state)) {
    return status.ui_state;
  }
  // Legacy inference — remove once all Pi gateways serve ui_state.
  if (status.deploy.state === 'running') return 'updating';
  if (status.deploy.state === 'failed') return 'failed';
  if (!status.upstream_ok) return 'upstream_unknown';
  if (status.update_available) return 'stale';
  if (status.deploy_sha) return 'current';
  return 'unknown';
}

function isBusyStatus(status: VersionStatusDto | null): boolean {
  if (!status) return false;
  if (status.ui_state === 'updating') return true;
  return status.deploy.state === 'running';
}

/** Polling + deploy mutation controller for sidebar self-update chrome. */
export function useSidebarSelfUpdate() {
  const [status, setStatus] = useState<VersionStatusDto | null>(null);
  const [checking, setChecking] = useState(false);
  const [confirmOpen, setConfirmOpen] = useState(false);
  const [deployBusy, setDeployBusy] = useState(false);
  const [watchingJob, setWatchingJob] = useState(
    () => Boolean(readSelfUpdateSession()),
  );
  const [stickyFailed, setStickyFailed] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const reloadArmed = useRef(false);

  const mode = deriveSidebarUpdateMode(status, {
    deployBusy,
    watchingJob,
    stickyFailed,
  });
  const shaLabel = status?.deploy_sha ? shortSha(status.deploy_sha) : '—';
  const phase = mode === 'updating' ? phaseLabel(status?.deploy.phase) : null;
  const caption = statusCaption(mode, shaLabel, phase);
  // Stale (behind) or Failed (retry / www repair) — never hide Update behind sticky failure.
  const showUpdate = (mode === 'stale' || mode === 'failed') && !checking;

  useEffect(() => {
    let cancelled = false;
    let timer: ReturnType<typeof setTimeout> | undefined;

    const stopWatching = () => {
      clearSelfUpdateSession();
      setWatchingJob(false);
    };

    const tick = async (refresh: boolean) => {
      let next: VersionStatusDto | null = null;
      const controller = new AbortController();
      const abortTimer = window.setTimeout(() => {
        controller.abort();
      }, POLL_FETCH_TIMEOUT_MS);
      try {
        next = await fetchVersionStatus({
          refresh,
          signal: controller.signal,
        });
        if (cancelled) return;
        setStatus(next);

        const sess = readSelfUpdateSession();
        if (sess) {
          const timedOut = Date.now() - sess.startedAtMs > SELF_UPDATE_TIMEOUT_MS;
          // Timeout even when status is null (gateway bounce / persistent unreachable).
          if (timedOut) {
            stopWatching();
            setStickyFailed(false);
            setError('Timed out — see /opt/marengo/var/self-update.log');
            toast.error('Update timed out');
          } else if (next) {
            const outcome = resolveWatchOutcome(next, sess);
            if (outcome === 'failed') {
              // Sticky Failed before clearing watch — same React turn as setStatus.
              setStickyFailed(true);
              stopWatching();
              const msg = next.deploy.message || 'Self-update failed';
              setError(msg);
              toast.error(msg);
            } else if (outcome === 'success' && !reloadArmed.current) {
              reloadArmed.current = true;
              setStickyFailed(false);
              stopWatching();
              // Stale running/enqueue ledger can still report ui_state=updating —
              // paint current immediately so “Updating · Queued” cannot linger until reload.
              setStatus({
                ...next,
                update_available: false,
                ready_for_target: true,
                ui_state: 'current',
                deploy: {
                  ...next.deploy,
                  state: 'succeeded',
                  phase: 'done',
                },
              });
              toast.success('Update complete');
              window.setTimeout(() => {
                window.location.reload();
              }, 400);
            }
          }
        }
      } catch {
        // Aborted / network error during gateway restart — keep watching; finally reschedules.
        if (cancelled) return;
      } finally {
        window.clearTimeout(abortTimer);
        if (!cancelled) {
          setWatchingJob(Boolean(readSelfUpdateSession()));
          const busy = Boolean(readSelfUpdateSession()) || isBusyStatus(next);
          timer = window.setTimeout(
            () => {
              void tick(false);
            },
            busy ? BUSY_POLL_MS : IDLE_POLL_MS,
          );
        }
      }
    };

    void tick(false);
    return () => {
      cancelled = true;
      if (timer) window.clearTimeout(timer);
    };
  }, []);

  const onCheck = () => {
    void (async () => {
      setChecking(true);
      setError(null);
      setStickyFailed(false);
      const next = await fetchVersionStatus({ refresh: true });
      setChecking(false);
      setStatus(next);
      if (!next) {
        toast.error('Gateway unreachable');
        return;
      }
      const state: UpdateUiState | undefined = next.ui_state;
      if (state === 'upstream_unknown' || (!state && !next.upstream_ok)) {
        toast.message('GitHub unreachable — showing installed rev');
        return;
      }
      if (state === 'failed' || next.deploy.state === 'failed') {
        toast.message(next.deploy.message || 'Last update failed — use Update to retry');
        return;
      }
      if (state === 'stale' || (!state && next.update_available)) {
        toast.message('Update available');
        return;
      }
      toast.info('Already up to date');
    })();
  };

  const openConfirm = () => {
    setError(null);
    setConfirmOpen(true);
  };

  const onConfirmUpdate = () => {
    void (async () => {
      setDeployBusy(true);
      setError(null);
      setStickyFailed(false);
      const result = await startSelfDeploy();
      if (result.already_current) {
        setDeployBusy(false);
        setConfirmOpen(false);
        toast.info('Already up to date');
        return;
      }
      if (!result.ok || !result.job_id) {
        setDeployBusy(false);
        setError(result.message);
        toast.error(result.message);
        return;
      }
      writeSelfUpdateSession({
        jobId: result.job_id,
        startedAtMs: Date.now(),
        ...(result.target_sha ? { targetSha: result.target_sha } : {}),
      });
      setWatchingJob(true);
      setDeployBusy(false);
      setConfirmOpen(false);
      toast.message('Updating Marengo…');
      const next = await fetchVersionStatus();
      setStatus(next);
    })();
  };

  return {
    status,
    mode,
    caption,
    shaTitle: status?.deploy_sha || undefined,
    checking,
    error,
    showUpdate,
    confirmOpen,
    deployBusy,
    setConfirmOpen,
    onCheck,
    openConfirm,
    onConfirmUpdate,
  };
}
