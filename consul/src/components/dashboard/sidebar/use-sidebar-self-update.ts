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
  shortSha,
  startSelfDeploy,
  writeSelfUpdateSession,
  type VersionStatusDto,
} from '@/lib/version-api';

const IDLE_POLL_MS = 60_000;
const BUSY_POLL_MS = 2_500;

export function deriveSidebarUpdateMode(
  status: VersionStatusDto | null,
  forceUpdating: boolean,
): SidebarUpdateUiMode {
  if (forceUpdating) return 'updating';
  if (!status) return 'unknown';
  if (status.deploy.state === 'running') return 'updating';
  if (status.deploy.state === 'failed') return 'failed';
  if (
    status.deploy.state === 'succeeded' &&
    status.ready_for_target &&
    readSelfUpdateSession()
  ) {
    return 'updating';
  }
  if (!status.upstream_ok) return 'upstream_unknown';
  if (status.update_available) return 'stale';
  if (status.deploy_sha) return 'current';
  return 'unknown';
}

/** Polling + deploy mutation controller for sidebar self-update chrome. */
export function useSidebarSelfUpdate() {
  const [status, setStatus] = useState<VersionStatusDto | null>(null);
  const [checking, setChecking] = useState(false);
  const [confirmOpen, setConfirmOpen] = useState(false);
  const [deployBusy, setDeployBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const reloadArmed = useRef(false);

  const session = readSelfUpdateSession();
  const forceUpdating = Boolean(session) || deployBusy;
  const mode = deriveSidebarUpdateMode(
    status,
    forceUpdating && status?.deploy.state !== 'failed',
  );
  const shaLabel = status?.deploy_sha ? shortSha(status.deploy_sha) : '—';
  const phase = mode === 'updating' ? phaseLabel(status?.deploy.phase) : null;
  const caption = statusCaption(mode, shaLabel, phase);
  const showUpdate = mode === 'stale' && !checking;

  useEffect(() => {
    let cancelled = false;
    let timer: ReturnType<typeof setTimeout> | undefined;

    const tick = async (refresh: boolean) => {
      const next = await fetchVersionStatus({ refresh });
      if (cancelled) return;
      setStatus(next);

      const sess = readSelfUpdateSession();
      if (sess && next) {
        const timedOut = Date.now() - sess.startedAtMs > SELF_UPDATE_TIMEOUT_MS;
        if (timedOut) {
          clearSelfUpdateSession();
          setError('Timed out — see /opt/marengo/var/self-update.log');
          toast.error('Update timed out');
        } else if (next.deploy.state === 'failed') {
          clearSelfUpdateSession();
          const msg = next.deploy.message || 'Self-update failed';
          setError(msg);
          toast.error(msg);
        } else if (
          next.deploy.state === 'succeeded' &&
          next.ready_for_target &&
          !reloadArmed.current
        ) {
          reloadArmed.current = true;
          clearSelfUpdateSession();
          toast.success('Update complete');
          window.setTimeout(() => {
            window.location.reload();
          }, 400);
        }
      }

      const busy =
        Boolean(readSelfUpdateSession()) || next?.deploy.state === 'running';
      timer = window.setTimeout(
        () => {
          void tick(false);
        },
        busy ? BUSY_POLL_MS : IDLE_POLL_MS,
      );
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
      const next = await fetchVersionStatus({ refresh: true });
      setChecking(false);
      setStatus(next);
      if (!next) {
        toast.error('Gateway unreachable');
        return;
      }
      if (!next.upstream_ok) {
        toast.message('GitHub unreachable — showing installed rev');
        return;
      }
      if (next.update_available) {
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
      const result = await startSelfDeploy();
      if (result.already_current) {
        setDeployBusy(false);
        setConfirmOpen(false);
        toast.info('Already up to date');
        return;
      }
      if (!result.ok || !result.job_id || !result.target_sha) {
        setDeployBusy(false);
        setError(result.message);
        toast.error(result.message);
        return;
      }
      writeSelfUpdateSession({
        jobId: result.job_id,
        targetSha: result.target_sha,
        startedAtMs: Date.now(),
      });
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
