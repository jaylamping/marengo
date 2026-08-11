import { useEffect, useRef, useState } from 'react';
import { Loading03Icon, RefreshIcon } from '@hugeicons/core-free-icons';
import { HugeiconsIcon } from '@hugeicons/react';
import { toast } from 'sonner';

import { Button } from '@/components/ui/button';
import {
  Dialog,
  DialogBody,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from '@/components/ui/dialog';
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

type UiMode = 'unknown' | 'current' | 'stale' | 'upstream_unknown' | 'updating' | 'failed';

function deriveMode(status: VersionStatusDto | null, forceUpdating: boolean): UiMode {
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

export function SidebarUpdateStatus() {
  const [status, setStatus] = useState<VersionStatusDto | null>(null);
  const [checking, setChecking] = useState(false);
  const [confirmOpen, setConfirmOpen] = useState(false);
  const [deployBusy, setDeployBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const reloadArmed = useRef(false);

  const session = readSelfUpdateSession();
  const forceUpdating = Boolean(session) || deployBusy;
  const mode = deriveMode(status, forceUpdating && status?.deploy.state !== 'failed');

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
          setError('Update timed out — check Pi logs /opt/marengo/var/self-update.log');
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
        Boolean(readSelfUpdateSession()) ||
        next?.deploy.state === 'running' ||
        (next?.deploy.state === 'succeeded' && Boolean(readSelfUpdateSession()));
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
        toast.error('Could not reach gateway version status');
        return;
      }
      if (!next.upstream_ok) {
        toast.message('Installed rev shown — GitHub unreachable');
        return;
      }
      if (next.update_available) {
        toast.message('Update available');
        return;
      }
      toast.info('Already up to date');
    })();
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

  const shaLabel = status?.deploy_sha ? shortSha(status.deploy_sha) : '—';
  const updating = mode === 'updating';
  const showUpdate = mode === 'stale' && !updating && !checking;

  return (
    <div className="flex w-full flex-col gap-1.5 px-2 pb-1" data-testid="sidebar-update-status">
      <div className="flex items-center justify-between gap-2 font-mono text-[10px] text-muted-foreground">
        <span className="truncate" title={status?.deploy_sha || undefined}>
          {updating ? 'Updating…' : `rev ${shaLabel}`}
          {mode === 'stale' ? ' · stale' : null}
          {mode === 'upstream_unknown' ? ' · upstream?' : null}
          {mode === 'failed' ? ' · failed' : null}
        </span>
        {updating ? (
          <HugeiconsIcon
            icon={Loading03Icon}
            strokeWidth={2}
            className="size-3.5 shrink-0 animate-spin text-sky-400"
            data-testid="sidebar-update-spinner"
          />
        ) : null}
      </div>
      {error ? (
        <p className="text-[10px] leading-snug text-fault" role="alert">
          {error}
        </p>
      ) : null}
      <div className="flex flex-wrap gap-1">
        <Button
          type="button"
          variant="outline"
          size="xs"
          disabled={updating || checking}
          data-testid="check-for-updates"
          onClick={onCheck}
        >
          {checking ? (
            <HugeiconsIcon
              icon={Loading03Icon}
              strokeWidth={2}
              className="size-3 animate-spin"
              data-icon="inline-start"
            />
          ) : (
            <HugeiconsIcon
              icon={RefreshIcon}
              strokeWidth={2}
              className="size-3"
              data-icon="inline-start"
            />
          )}
          Check for updates
        </Button>
        {showUpdate ? (
          <Button
            type="button"
            size="xs"
            className="border-sky-600/40 bg-sky-600 text-white hover:bg-sky-500"
            data-testid="sidebar-update-button"
            onClick={() => {
              setError(null);
              setConfirmOpen(true);
            }}
          >
            Update
          </Button>
        ) : null}
      </div>

      <Dialog
        open={confirmOpen}
        onOpenChange={(next) => {
          if (!next && !deployBusy) setConfirmOpen(false);
        }}
      >
        <DialogContent
          variant="default"
          showCloseButton={!deployBusy}
          className="max-w-md"
          data-testid="update-confirm-dialog"
        >
          <DialogHeader>
            <DialogTitle>Update Marengo now?</DialogTitle>
            <DialogDescription>
              Pulls the latest GitHub main onto the Pi, builds natively, and installs to
              /opt/marengo. Expect several minutes of downtime. Support elevated arms —
              motors go limp during install.
            </DialogDescription>
          </DialogHeader>
          <DialogBody>
            {status?.upstream_sha ? (
              <p className="font-mono text-xs text-muted-foreground">
                Target {shortSha(status.upstream_sha)}
                {status.deploy_sha ? ` ← ${shortSha(status.deploy_sha)}` : null}
              </p>
            ) : null}
          </DialogBody>
          <DialogFooter>
            <Button
              type="button"
              variant="outline"
              size="sm"
              disabled={deployBusy}
              onClick={() => setConfirmOpen(false)}
            >
              Cancel
            </Button>
            <Button
              type="button"
              size="sm"
              className="border-sky-600/40 bg-sky-600 text-white hover:bg-sky-500"
              disabled={deployBusy}
              data-testid="confirm-update-button"
              onClick={onConfirmUpdate}
            >
              {deployBusy ? 'Starting…' : 'Update now'}
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>
    </div>
  );
}
