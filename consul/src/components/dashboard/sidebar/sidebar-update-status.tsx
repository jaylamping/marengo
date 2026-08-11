import { useEffect, useRef, useState } from 'react';
import { Loading03Icon } from '@hugeicons/core-free-icons';
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
  Avatar,
  AvatarFallback,
  AvatarImage,
} from '@/components/ui/avatar';
import {
  SidebarUpdateButton,
  SidebarUpdateStatusView,
  phaseLabel,
  statusCaption,
  type SidebarUpdateUiMode,
} from '@/components/dashboard/sidebar/sidebar-update-status-view';
import type { SidebarUser } from '@/data/sidebar-nav';
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

function deriveMode(
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

function getInitials(name: string): string {
  return name
    .split(' ')
    .map((part) => part[0]?.toUpperCase() ?? '')
    .join('')
    .slice(0, 2);
}

type SidebarUpdateStatusProps = {
  user: SidebarUser;
};

/** Identity row (Update chip when stale) + rev/Check status under it. */
export function SidebarUpdateStatus({ user }: SidebarUpdateStatusProps) {
  const [status, setStatus] = useState<VersionStatusDto | null>(null);
  const [checking, setChecking] = useState(false);
  const [confirmOpen, setConfirmOpen] = useState(false);
  const [deployBusy, setDeployBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const reloadArmed = useRef(false);

  const session = readSelfUpdateSession();
  const forceUpdating = Boolean(session) || deployBusy;
  const mode = deriveMode(status, forceUpdating && status?.deploy.state !== 'failed');
  const shaLabel = status?.deploy_sha ? shortSha(status.deploy_sha) : '—';
  const phase = mode === 'updating' ? phaseLabel(status?.deploy.phase) : null;
  const caption = statusCaption(mode, shaLabel, phase);
  const showUpdate = mode === 'stale' && !checking;
  const initials = getInitials(user.name);

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
    <>
      <div className="flex w-full flex-col">
        <div className="flex w-full items-center gap-2 rounded-md px-2 py-1.5">
          <Avatar className="size-8 rounded-lg grayscale">
            <AvatarImage src={user.avatar} alt={user.name} />
            <AvatarFallback className="rounded-lg">{initials}</AvatarFallback>
          </Avatar>
          <div className="grid min-w-0 flex-1 text-left text-sm leading-tight">
            <span className="truncate font-medium">{user.name}</span>
            <span className="truncate font-mono text-xs text-muted-foreground">
              {user.context}
            </span>
          </div>
          {showUpdate ? (
            <SidebarUpdateButton
              onClick={() => {
                setError(null);
                setConfirmOpen(true);
              }}
            />
          ) : null}
        </div>
        <SidebarUpdateStatusView
          mode={mode}
          caption={caption}
          shaTitle={status?.deploy_sha || undefined}
          checking={checking}
          error={error}
          onCheck={onCheck}
        />
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
    </>
  );
}
