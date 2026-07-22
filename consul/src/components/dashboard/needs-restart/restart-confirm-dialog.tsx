import { useState } from 'react';

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
import { restartMarengoPi } from '@/lib/config-api';
import { useNeedsRestartStore } from '@/state/needsRestartStore';
import { useRobotStore } from '@/state/robotStore';

/**
 * Shared “Would you like to restart now?” dialog for Set Limits Apply and all
 * Needs restart / Pending restart badges.
 */
export function RestartConfirmDialog() {
  const open = useNeedsRestartStore((s) => s.restartDialogOpen);
  const fromApply = useNeedsRestartStore((s) => s.dialogFromApply);
  const closeRestartDialog = useNeedsRestartStore((s) => s.closeRestartDialog);
  const clearNeedsRestart = useNeedsRestartStore((s) => s.clearNeedsRestart);
  const operationalMode = useRobotStore((s) => s.operationalMode);
  const connected = useRobotStore((s) => s.connected);

  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const activeBlocks =
    connected && operationalMode === 'ACTIVE';

  return (
    <Dialog
      open={open}
      onOpenChange={(next) => {
        if (!next && !busy) {
          setError(null);
          closeRestartDialog();
        }
      }}
    >
      <DialogContent
        variant="default"
        showCloseButton={!busy}
        className="max-w-md"
        data-testid="restart-confirm-dialog"
      >
        <DialogHeader>
          <DialogTitle>Would you like to restart now?</DialogTitle>
          <DialogDescription>
            {fromApply
              ? 'Hard limits were saved to motors.yaml. Restart marengo-pi so Davout loads them.'
              : 'Restart marengo-pi so Davout reloads hard limits from motors.yaml.'}{' '}
            Motors will go limp — support elevated arms.
          </DialogDescription>
        </DialogHeader>
        <DialogBody className="space-y-2">
          {activeBlocks ? (
            <p className="text-xs text-fault" data-testid="restart-active-block">
              Motors are ACTIVE. Disable before restarting the control loop.
            </p>
          ) : null}
          {error ? (
            <p className="text-xs text-fault" role="alert">
              {error}
            </p>
          ) : null}
        </DialogBody>
        <DialogFooter>
          <Button
            type="button"
            variant="outline"
            size="sm"
            disabled={busy}
            onClick={() => {
              setError(null);
              closeRestartDialog();
            }}
          >
            Later
          </Button>
          <Button
            type="button"
            size="sm"
            disabled={busy || activeBlocks}
            data-testid="restart-now-button"
            onClick={() => {
              void (async () => {
                setBusy(true);
                setError(null);
                const result = await restartMarengoPi();
                setBusy(false);
                if (!result.ok) {
                  setError(result.message);
                  return;
                }
                clearNeedsRestart();
              })();
            }}
          >
            {busy ? 'Restarting…' : 'Restart now'}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
