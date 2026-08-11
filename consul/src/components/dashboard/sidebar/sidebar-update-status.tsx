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
import { cn } from '@/lib/utils';
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

function phaseLabel(phase: string | undefined): string | null {
  if (!phase) return null;
  const map: Record<string, string> = {
    init: 'Init',
    dirty: 'Dirty tree',
    fetch: 'Fetch',
    lfs: 'LFS',
    build: 'Build',
    install: 'Install',
    enqueue: 'Queued',
    done: 'Done',
    timeout: 'Timed out',
    orphan: 'Interrupted',
    error: 'Error',
  };
  return map[phase] ?? phase;
}

function statusLedClass(mode: UiMode): string {
  switch (mode) {
    case 'current':
      return 'led led-ok';
    case 'stale':
      return 'led led-info';
    case 'failed':
      return 'led led-fault';
    case 'updating':
      return 'led led-info led-live';
    case 'upstream_unknown':
    case 'unknown':
      return 'led';
    default: {
      const _exhaustive: never = mode;
      return _exhaustive;
    }
  }
}

function statusCaption(mode: UiMode, shaLabel: string, phase: string | null): string {
  switch (mode) {
    case 'updating':
      return phase ? `Updating · ${phase}` : 'Updating…';
    case 'stale':
      return `rev ${shaLabel} · behind`;
    case 'upstream_unknown':
      return `rev ${shaLabel} · offline`;
    case 'failed':
      return `rev ${shaLabel} · failed`;
    case 'current':
      return `rev ${shaLabel}`;
    case 'unknown':
      return 'rev —';
    default: {
      const _exhaustive: never = mode;
      return _exhaustive;
    }
  }
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
  const updating = mode === 'updating';
  const showUpdate = mode === 'stale' && !updating && !checking;
  const shaLabel = status?.deploy_sha ? shortSha(status.deploy_sha) : '—';
  const phase = updating ? phaseLabel(status?.deploy.phase) : null;
  const caption = statusCaption(mode, shaLabel, phase);

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

  return (
    <div
      className="flex w-full flex-col gap-1.5 border-t border-line px-2 pt-2"
      data-testid="sidebar-update-status"
    >
      <div className="flex min-w-0 items-center gap-1.5">
        <span className={statusLedClass(mode)} aria-hidden />
        <span
          className="micro-label min-w-0 flex-1 truncate normal-case tracking-normal"
          title={status?.deploy_sha || undefined}
        >
          {caption}
        </span>
        {updating ? (
          <HugeiconsIcon
            icon={Loading03Icon}
            strokeWidth={2}
            className="size-3.5 shrink-0 animate-spin text-info motion-reduce:animate-none"
            data-testid="sidebar-update-spinner"
            aria-hidden
          />
        ) : null}
      </div>

      {error ? (
        <p className="text-[10px] leading-snug text-fault" role="alert">
          {error}
        </p>
      ) : null}

      <div className="flex flex-wrap items-center gap-1">
        <Button
          type="button"
          variant="ghost"
          size="xs"
          className="h-6 px-1.5 text-muted-foreground hover:text-foreground"
          disabled={updating || checking}
          data-testid="check-for-updates"
          aria-label="Check for updates"
          title="Check for updates"
          onClick={onCheck}
        >
          {checking ? (
            <HugeiconsIcon
              icon={Loading03Icon}
              strokeWidth={2}
              className="size-3 animate-spin motion-reduce:animate-none"
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
          Check
        </Button>

        {showUpdate ? (
          <button
            type="button"
            data-testid="sidebar-update-button"
            className={cn(
              'inline-flex h-6 shrink-0 items-center border border-info/50 bg-info/10 px-2 font-mono text-[10px] uppercase tracking-[0.14em] text-info transition-colors',
              'hover:bg-info/20 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring',
            )}
            title="Install latest main onto the Pi"
            onClick={() => {
              setError(null);
              setConfirmOpen(true);
            }}
          >
            Update
          </button>
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
            <DialogTitle>Update Marengo?</DialogTitle>
            <DialogDescription>
              Pins GitHub main on the Pi, builds natively, and installs to /opt/marengo.
              Several minutes of downtime. Support elevated arms — motors go limp during
              install.
            </DialogDescription>
          </DialogHeader>
          <DialogBody className="space-y-2">
            {status?.upstream_sha ? (
              <p className="font-mono text-xs text-muted-foreground">
                <span className="text-info">{shortSha(status.upstream_sha)}</span>
                {status.deploy_sha ? (
                  <>
                    <span className="text-faint"> ← </span>
                    {shortSha(status.deploy_sha)}
                  </>
                ) : null}
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
              className="border-info/40 bg-info/90 text-background hover:bg-info"
              disabled={deployBusy}
              data-testid="confirm-update-button"
              onClick={onConfirmUpdate}
            >
              {deployBusy ? (
                <>
                  <HugeiconsIcon
                    icon={Loading03Icon}
                    strokeWidth={2}
                    className="size-3.5 animate-spin motion-reduce:animate-none"
                    data-icon="inline-start"
                  />
                  Starting…
                </>
              ) : (
                'Update now'
              )}
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>
    </div>
  );
}
