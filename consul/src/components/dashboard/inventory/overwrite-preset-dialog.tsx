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

export type OverwritePresetLimits = {
  position_lower_rad: number;
  position_upper_rad: number;
};

type OverwritePresetDialogProps = {
  open: boolean;
  joint: string;
  presetId: string;
  expectedRevision: string;
  before: OverwritePresetLimits | null;
  after: OverwritePresetLimits | null;
  busy?: boolean;
  error?: string | null;
  onCancel: () => void;
  onConfirm: () => void;
};

function fmt(n: number): string {
  return n.toFixed(3);
}

export function OverwritePresetActuatorDialog({
  open,
  joint,
  presetId,
  expectedRevision,
  before,
  after,
  busy = false,
  error = null,
  onCancel,
  onConfirm,
}: OverwritePresetDialogProps) {
  return (
    <Dialog
      open={open}
      onOpenChange={(next) => {
        if (!next && !busy) {
          onCancel();
        }
      }}
    >
      <DialogContent
        className="max-w-md"
        data-testid="overwrite-preset-dialog"
        showCloseButton={!busy}
      >
        <DialogHeader>
          <DialogTitle>Overwrite {presetId} limits?</DialogTitle>
          <DialogDescription>
            Copy live limits for {joint} into the bringup profile. CAS revision{' '}
            <span className="font-mono text-[10px]">{expectedRevision.slice(0, 8)}</span>
            …
          </DialogDescription>
        </DialogHeader>
        <DialogBody className="space-y-2 font-mono text-xs">
          {before ? (
            <p>
              Before: [{fmt(before.position_lower_rad)},{' '}
              {fmt(before.position_upper_rad)}]
            </p>
          ) : (
            <p>Before: (joint not in target)</p>
          )}
          {after ? (
            <p>
              After: [{fmt(after.position_lower_rad)},{' '}
              {fmt(after.position_upper_rad)}]
            </p>
          ) : null}
          {error ? (
            <p className="text-fault" role="alert">
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
            onClick={onCancel}
          >
            Cancel
          </Button>
          <Button
            type="button"
            size="sm"
            disabled={busy}
            data-testid="overwrite-preset-confirm"
            onClick={onConfirm}
          >
            {busy ? 'Applying…' : 'Overwrite'}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
