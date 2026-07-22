import * as React from 'react';
import type { AutoLearnStage } from '@marengo/compound-auto-learn';
import { dashboardPanelCardClassName } from '@/components/dashboard/layout/constants';
import { Button } from '@/components/ui/button';
import { Checkbox } from '@/components/ui/checkbox';
import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from '@/components/ui/card';
import { useAutoLearnController } from '@/hooks/use-auto-learn-controller';
import { cn } from '@/lib/utils';

const STAGES: AutoLearnStage[] = ['crawl', 'walk', 'run'];

interface AutoLearnPanelProps {
  presetId: string;
  open: boolean;
  onOpenChange: (open: boolean) => void;
  /** Compound Playback Start — Dry Run / live hardware tests. */
  onStartPlayback: () => void;
}

export function AutoLearnPanel({
  presetId,
  open,
  onOpenChange,
  onStartPlayback,
}: AutoLearnPanelProps) {
  const {
    configured,
    movementBrief,
    stage,
    includeLogs,
    feedback,
    status,
    error,
    logAttachNote,
    reviewHint,
    draft,
    hasPrior,
    proposalTested,
    recording,
    compoundRunning,
    hardwareBlockReason,
    setStage,
    setIncludeLogs,
    setFeedback,
    clearFeedback,
    setLandmarkIncluded,
    autoLearn,
    advanceStage,
    testProposal,
    testOnHardware,
    applyOverlay,
    discard,
  } = useAutoLearnController(presetId);
  const [pendingAction, setPendingAction] = React.useState<
    null | 'generate' | 'advance'
  >(null);
  const [hardwareConfirmOpen, setHardwareConfirmOpen] = React.useState(false);

  const logsConfirm = error?.startsWith('LOGS_CONFIRM:') ?? false;
  const displayError = logsConfirm
    ? (error ?? '').replace(/^LOGS_CONFIRM:/, '')
    : error;

  React.useEffect(() => {
    if (logsConfirm && !pendingAction) {
      setPendingAction('generate');
    }
  }, [logsConfirm, pendingAction]);

  React.useEffect(() => {
    setHardwareConfirmOpen(false);
  }, [draft?.description, draft?.stage, presetId]);

  if (!open) return null;

  const busy = status === 'calling' || recording || compoundRunning;

  return (
    <Card variant="panel" className={dashboardPanelCardClassName}>
      <CardHeader className="flex flex-row items-start justify-between gap-4">
        <div>
          <CardTitle className="text-lg">Auto Learn</CardTitle>
          <CardDescription>
            Cursor proposes a stage-capped teach overlay. Test proposal (Dry
            Run), then Apply. Manual Movement is separate.
          </CardDescription>
        </div>
        <Button size="sm" variant="ghost" onClick={() => onOpenChange(false)}>
          Close
        </Button>
      </CardHeader>
      <CardContent className="space-y-4">
        {movementBrief ? (
          <p className="rounded-md border border-border/60 bg-muted/30 px-3 py-2 text-sm leading-relaxed text-muted-foreground">
            <span className="mb-1 block font-mono text-[10px] uppercase tracking-[0.14em] text-foreground/70">
              Movement brief
            </span>
            {movementBrief}
          </p>
        ) : null}

        <div className="flex flex-wrap items-center gap-3">
          <div className="flex gap-1">
            {STAGES.map((s) => {
              const disabled =
                busy || (s !== 'crawl' && !hasPrior) || !configured;
              return (
                <Button
                  key={s}
                  type="button"
                  size="sm"
                  variant={stage === s ? 'default' : 'outline'}
                  className="font-mono text-[10px] uppercase tracking-[0.14em]"
                  disabled={disabled}
                  onClick={() => setStage(s)}
                >
                  {s}
                </Button>
              );
            })}
          </div>
          <span className="font-mono text-[10px] uppercase tracking-[0.14em] text-muted-foreground">
            {status}
          </span>
        </div>

        <div className="flex flex-wrap items-center gap-3">
          <div className="flex items-center gap-2">
            <Checkbox
              id={`auto-learn-logs-${presetId}`}
              checked={includeLogs}
              disabled={busy}
              onCheckedChange={(v) => setIncludeLogs(!!v)}
            />
            <label
              htmlFor={`auto-learn-logs-${presetId}`}
              className="cursor-pointer text-sm"
            >
              Include session logs
            </label>
          </div>
          {logAttachNote ? (
            <span className="font-mono text-xs text-muted-foreground">
              {logAttachNote}
            </span>
          ) : includeLogs ? (
            <span className="font-mono text-xs text-muted-foreground">
              will attach on next call
            </span>
          ) : null}
        </div>

        <div className="space-y-2">
          <div className="flex items-center justify-between gap-2">
            <label
              htmlFor={`auto-learn-feedback-${presetId}`}
              className="text-sm font-medium"
            >
              Feedback
            </label>
            <Button
              type="button"
              size="sm"
              variant="ghost"
              className="h-7 text-xs"
              disabled={!feedback}
              onClick={() => clearFeedback()}
            >
              Clear feedback
            </Button>
          </div>
          <textarea
            id={`auto-learn-feedback-${presetId}`}
            className={cn(
              'min-h-[4.5rem] w-full resize-y rounded-md border border-border/60 bg-muted/20',
              'px-3 py-2 font-mono text-xs text-foreground placeholder:text-muted-foreground',
              'focus-visible:outline-none focus-visible:ring-1 focus-visible:ring-ring',
            )}
            placeholder="What should change after the last test?"
            value={feedback}
            maxLength={2048}
            disabled={busy}
            onChange={(e) => setFeedback(e.target.value)}
          />
        </div>

        <div className="flex flex-wrap gap-2">
          <Button
            type="button"
            disabled={!configured || busy}
            onClick={() => {
              setPendingAction('generate');
              void autoLearn();
            }}
          >
            {status === 'calling'
              ? 'Calling…'
              : hasPrior
                ? 'Regenerate'
                : 'Auto Learn'}
          </Button>
          <Button
            type="button"
            variant="outline"
            disabled={!hasPrior || busy || !configured}
            onClick={() => {
              setPendingAction('advance');
              void advanceStage();
            }}
          >
            Advance Stage
          </Button>
        </div>

        {!configured ? (
          <p className="font-mono text-xs text-destructive">
            Set VITE_AUTO_LEARN_URL and VITE_AUTO_LEARN_TOKEN
          </p>
        ) : null}

        {logsConfirm ? (
          <div className="space-y-2 rounded-md border border-border/60 bg-muted/20 p-3">
            <p className="font-mono text-xs text-amber-600 dark:text-amber-400">
              {displayError}. Continue without logs?
            </p>
            <div className="flex gap-2">
              <Button
                size="sm"
                onClick={() => {
                  if (pendingAction === 'advance') {
                    void advanceStage(true);
                  } else {
                    void autoLearn(true);
                  }
                  setPendingAction(null);
                }}
              >
                Continue without logs
              </Button>
              <Button
                size="sm"
                variant="outline"
                onClick={() => setPendingAction(null)}
              >
                Cancel
              </Button>
            </div>
          </div>
        ) : displayError ? (
          <p className="font-mono text-xs text-destructive">{displayError}</p>
        ) : null}

        {draft ? (
          <div className="space-y-3 border-t border-border/50 pt-4">
            <div className="flex flex-wrap items-baseline justify-between gap-2">
              <h4 className="text-sm font-semibold uppercase tracking-wider text-muted-foreground">
                Review
              </h4>
              <span className="font-mono text-[10px] uppercase tracking-[0.14em] text-muted-foreground">
                {draft.stage} ·{' '}
                {draft.landmarks.filter((l) => l.included).length} landmarks ·
                speed≤{draft.speedMultiplier.toFixed(2)}
              </span>
            </div>
            <p className="text-sm text-muted-foreground">{draft.description}</p>
            <ul className="space-y-1">
              {draft.landmarks.map((lm) => (
                <li
                  key={lm.id}
                  className="flex items-center gap-2 font-mono text-xs"
                >
                  <Checkbox
                    checked={lm.included}
                    onCheckedChange={(v) => setLandmarkIncluded(lm.id, !!v)}
                    id={`al-lm-${lm.id}`}
                  />
                  <label htmlFor={`al-lm-${lm.id}`} className="cursor-pointer">
                    {lm.label} · t={lm.tSec.toFixed(2)}s
                  </label>
                </li>
              ))}
            </ul>
            <p className="font-mono text-[10px] uppercase tracking-[0.14em] text-muted-foreground">
              Test proposal = Dry Run · Test on hardware = live motors · Apply
              commits the overlay
            </p>
            {hardwareBlockReason ? (
              <p
                className="font-mono text-xs text-amber-600 dark:text-amber-400"
                role="status"
              >
                {hardwareBlockReason}
              </p>
            ) : null}
            {reviewHint ? (
              <p className="font-mono text-xs text-amber-600 dark:text-amber-400">
                {reviewHint}
              </p>
            ) : null}
            {hardwareConfirmOpen ? (
              <div className="space-y-2 rounded-md border border-border/60 bg-muted/20 p-3">
                <p className="font-mono text-xs text-amber-600 dark:text-amber-400">
                  Live motors will move. Arm supported, Dry Run off, stage speed
                  ceiling still applies. Continue?
                </p>
                <div className="flex flex-wrap gap-2">
                  <Button
                    type="button"
                    size="sm"
                    variant="destructive"
                    disabled={busy || !!hardwareBlockReason}
                    onClick={() => {
                      setHardwareConfirmOpen(false);
                      testOnHardware(onStartPlayback);
                    }}
                  >
                    Start live test
                  </Button>
                  <Button
                    type="button"
                    size="sm"
                    variant="outline"
                    disabled={busy}
                    onClick={() => setHardwareConfirmOpen(false)}
                  >
                    Cancel
                  </Button>
                </div>
              </div>
            ) : null}
            <div className="flex flex-wrap gap-2">
              <Button
                type="button"
                disabled={busy || hardwareConfirmOpen}
                onClick={() => testProposal(onStartPlayback)}
              >
                Test proposal
              </Button>
              <Button
                type="button"
                variant="destructive"
                disabled={busy || hardwareConfirmOpen || !!hardwareBlockReason}
                onClick={() => setHardwareConfirmOpen(true)}
              >
                Test on hardware
              </Button>
              <Button
                type="button"
                variant={proposalTested ? 'default' : 'outline'}
                disabled={busy || hardwareConfirmOpen}
                onClick={() => applyOverlay()}
              >
                Apply overlay
              </Button>
              <Button
                type="button"
                variant="outline"
                disabled={busy || hardwareConfirmOpen}
                onClick={() => discard()}
              >
                Discard
              </Button>
            </div>
          </div>
        ) : null}
      </CardContent>
    </Card>
  );
}
