import { useEffect, useEffectEvent, useId, useState } from 'react';

import { InformationCircleIcon } from '@hugeicons/core-free-icons';
import { HugeiconsIcon } from '@hugeicons/react';

import { Badge } from '@/components/ui/badge';
import { Button } from '@/components/ui/button';
import { Separator } from '@/components/ui/separator';
import {
  Tooltip,
  TooltipContent,
  TooltipTrigger,
} from '@/components/ui/tooltip';
import { postSetZeroCommand } from '@/lib/gateway-api';
import {
  canStartLimitListen,
  limitListenBlockReason,
} from '@/lib/limit-listen';
import { subscribeTeachSamples } from '@/lib/teach-sample-bus';
import { useLimitListenStore } from '@/state/limitListenStore';
import { useRobotStore } from '@/state/robotStore';
import { useTeachStore } from '@/state/teachStore';

const SET_LIMITS_HELP =
  'Motors must be disabled (not ACTIVE) for Set Limits. Support the assembly, then sweep the joint to both hard stops while Consul samples position. Stop to propose min/max, then Apply to update Range. Set Zero briefly enables for firmware zero at the current pose, then disables again.';

type SetLimitsPanelProps = {
  jointName: string;
  currentLimit: string;
  onApplyRange: (range: string) => void;
};

/**
 * Actuator-only free-drive listen: operator supports and sweeps the joint with
 * motors not ACTIVE. Propose min/max for Apply onto the inventory Range field.
 */
export function SetLimitsPanel({
  jointName,
  currentLimit,
  onApplyRange,
}: SetLimitsPanelProps) {
  const connected = useRobotStore((s) => s.connected);
  const operationalMode = useRobotStore((s) => s.operationalMode);

  const phase = useLimitListenStore((s) => s.phase);
  const activeJoint = useLimitListenStore((s) => s.jointName);
  const bounds = useLimitListenStore((s) => s.bounds);
  const proposedRange = useLimitListenStore((s) => s.proposedRange);
  const error = useLimitListenStore((s) => s.error);
  const start = useLimitListenStore((s) => s.start);
  const ingestPosition = useLimitListenStore((s) => s.ingestPosition);
  const stop = useLimitListenStore((s) => s.stop);
  const abort = useLimitListenStore((s) => s.abort);
  const discard = useLimitListenStore((s) => s.discard);
  const reset = useLimitListenStore((s) => s.reset);
  const markCalibrationChanged = useTeachStore((s) => s.markCalibrationChanged);

  const [zeroBusy, setZeroBusy] = useState(false);
  const [zeroError, setZeroError] = useState<string | null>(null);
  const [zeroOk, setZeroOk] = useState(false);
  const [zeroConfirmOpen, setZeroConfirmOpen] = useState(false);

  const confirmTitleId = useId();
  const confirmDescId = useId();

  const isThisSession = activeJoint === jointName;
  const listening = phase === 'listening' && isThisSession;
  const reviewing = phase === 'review' && isThisSession;

  const gate = { connected, operationalMode };
  const canStart = canStartLimitListen(gate);
  const blockReason = limitListenBlockReason(gate);
  const canSetZero = connected && !listening && !zeroBusy && !reviewing;

  const onLiveSample = useEffectEvent((name: string, position: number) => {
    ingestPosition(name, position);
  });

  useEffect(() => {
    return () => {
      const state = useLimitListenStore.getState();
      if (state.jointName === jointName) {
        reset();
      }
    };
  }, [jointName, reset]);

  useEffect(() => {
    if (!listening) {
      return;
    }
    return subscribeTeachSamples((sample) => {
      const position = sample.q[jointName];
      if (position === undefined) {
        return;
      }
      onLiveSample(jointName, position);
    });
  }, [listening, jointName, onLiveSample]);

  useEffect(() => {
    if (!listening) {
      return;
    }
    if (!connected) {
      abort('Listen stopped — Chappe disconnected.');
      return;
    }
    if (operationalMode === 'ACTIVE') {
      abort('Listen stopped — motors went ACTIVE (disable and support the arm).');
    }
  }, [listening, connected, operationalMode, abort]);

  useEffect(() => {
    if (!zeroOk) {
      return;
    }
    const timer = window.setTimeout(() => setZeroOk(false), 4000);
    return () => window.clearTimeout(timer);
  }, [zeroOk]);

  useEffect(() => {
    if (!zeroConfirmOpen) {
      return;
    }
    const onKeyDown = (event: KeyboardEvent) => {
      if (event.key === 'Escape' && !zeroBusy) {
        setZeroConfirmOpen(false);
      }
    };
    window.addEventListener('keydown', onKeyDown);
    return () => window.removeEventListener('keydown', onKeyDown);
  }, [zeroConfirmOpen, zeroBusy]);

  const liveMin =
    bounds.sampleCount > 0 && Number.isFinite(bounds.min)
      ? bounds.min.toFixed(3)
      : '—';
  const liveMax =
    bounds.sampleCount > 0 && Number.isFinite(bounds.max)
      ? bounds.max.toFixed(3)
      : '—';
  const livePos =
    bounds.lastPosition !== null ? bounds.lastPosition.toFixed(3) : '—';

  const runSetZero = async () => {
    setZeroBusy(true);
    setZeroError(null);
    setZeroOk(false);
    try {
      await postSetZeroCommand(jointName);
      markCalibrationChanged();
      setZeroOk(true);
      setZeroConfirmOpen(false);
    } catch (e) {
      setZeroError(e instanceof Error ? e.message : 'Set Zero failed');
    } finally {
      setZeroBusy(false);
    }
  };

  return (
    <section
      className="flex flex-col gap-3 rounded-sm border border-line p-3 panel-brackets"
      data-testid="set-limits-panel"
      aria-label="Set joint limits"
    >
      <div className="flex items-center justify-between gap-2">
        <div className="flex items-center gap-1">
          <h3 className="font-mono text-[10px] uppercase tracking-[0.14em] text-muted-foreground">
            Set Limits
          </h3>
          <Tooltip>
            <TooltipTrigger
              render={
                <button
                  type="button"
                  className="inline-flex size-5 shrink-0 items-center justify-center rounded-sm text-muted-foreground transition-colors hover:text-foreground focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring"
                  aria-label="Set Limits help"
                >
                  <HugeiconsIcon
                    icon={InformationCircleIcon}
                    strokeWidth={2}
                    className="size-3.5"
                  />
                </button>
              }
            />
            <TooltipContent
              side="bottom"
              align="start"
              className="max-w-xs text-left leading-relaxed"
            >
              {SET_LIMITS_HELP}
            </TooltipContent>
          </Tooltip>
        </div>
        {listening ? (
          <Badge className="border-accent/40 bg-accent/15 font-mono text-[10px] uppercase tracking-[0.14em] text-accent">
            Listening
          </Badge>
        ) : reviewing ? (
          <Badge
            variant="secondary"
            className="font-mono text-[10px] uppercase tracking-[0.14em]"
          >
            Review
          </Badge>
        ) : (
          <Badge
            variant="outline"
            className="font-mono text-[10px] uppercase tracking-[0.14em]"
          >
            Idle
          </Badge>
        )}
      </div>

      <div className="flex flex-wrap gap-2 font-mono text-[10px] uppercase tracking-[0.14em] text-muted-foreground">
        <Badge variant="secondary" className="font-mono text-[10px] tracking-[0.14em]">
          mode {operationalMode ?? '—'}
        </Badge>
        <Badge variant="secondary" className="font-mono text-[10px] tracking-[0.14em]">
          link {connected ? 'live' : 'offline'}
        </Badge>
        <Badge variant="secondary" className="font-mono text-[10px] tracking-[0.14em]">
          samples {bounds.sampleCount}
        </Badge>
      </div>

      <div className="grid grid-cols-3 gap-2 font-mono text-xs">
        <MetricCell
          label="Pos"
          value={livePos}
          live={listening && bounds.lastPosition !== null}
        />
        <MetricCell label="Min" value={liveMin} live={listening && bounds.sampleCount > 0} />
        <MetricCell label="Max" value={liveMax} live={listening && bounds.sampleCount > 0} />
      </div>

      {error && isThisSession ? (
        <p className="text-xs text-fault" role="status">
          {error}
        </p>
      ) : null}

      {!canStart && !listening && !reviewing ? (
        <p className="text-xs text-muted-foreground" role="status">
          {blockReason}
        </p>
      ) : null}

      {reviewing && proposedRange ? (
        <div className="rounded-sm border border-accent/40 bg-accent/10 px-3 py-2 text-xs">
          <div className="font-mono text-[10px] uppercase tracking-[0.14em] text-accent">
            Proposed range
          </div>
          <div className="mt-1 font-mono tabular-nums">
            <span className="text-muted-foreground">{currentLimit}</span>
            <span className="mx-2 text-muted-foreground">→</span>
            <span className="text-accent">{proposedRange}</span>
          </div>
        </div>
      ) : null}

      {zeroError ? (
        <p className="text-xs text-fault" role="status">
          {zeroError}
        </p>
      ) : null}
      {zeroOk ? (
        <p className="text-xs text-ok" role="status">
          Set Zero sent — verify pos near 0, then Set Limits while disabled.
        </p>
      ) : null}

      {zeroConfirmOpen ? (
        <div
          className="flex flex-col gap-3 rounded-sm border border-fault/40 bg-fault/10 px-3 py-2"
          role="alertdialog"
          aria-modal="true"
          aria-labelledby={confirmTitleId}
          aria-describedby={confirmDescId}
        >
          <div className="flex flex-col gap-1">
            <div
              id={confirmTitleId}
              className="text-sm font-medium text-foreground"
            >
              Set Zero for{' '}
              <span className="font-mono text-fault">{jointName}</span>?
            </div>
            <p id={confirmDescId} className="text-xs text-fault/80">
              Replaces firmware zero at the current pose. Support the assembly
              at mechanical home before continuing.
            </p>
          </div>
          <div className="flex flex-wrap justify-end gap-2">
            <Button
              type="button"
              variant="outline"
              size="sm"
              disabled={zeroBusy}
              autoFocus
              onClick={() => setZeroConfirmOpen(false)}
            >
              Cancel
            </Button>
            <Button
              type="button"
              variant="destructive"
              size="sm"
              disabled={zeroBusy}
              onClick={() => {
                void runSetZero();
              }}
            >
              {zeroBusy ? 'Zeroing…' : 'Confirm Set Zero'}
            </Button>
          </div>
        </div>
      ) : null}

      <Separator className="bg-line" />

      <div className="flex flex-wrap items-center justify-between gap-2">
        <div className="flex flex-wrap gap-2">
          {listening ? (
            <Button type="button" variant="outline" size="sm" onClick={() => stop()}>
              Stop
            </Button>
          ) : reviewing ? (
            <>
              <Button
                type="button"
                size="sm"
                className="bg-accent text-accent-foreground hover:bg-accent/90"
                onClick={() => {
                  if (!proposedRange) {
                    return;
                  }
                  onApplyRange(proposedRange);
                  discard();
                }}
              >
                Apply Limits
              </Button>
              <Button
                type="button"
                variant="outline"
                size="sm"
                onClick={() => discard()}
              >
                Discard
              </Button>
            </>
          ) : (
            <Button
              type="button"
              size="sm"
              disabled={!canStart || zeroConfirmOpen}
              onClick={() => {
                setZeroOk(false);
                setZeroError(null);
                start(jointName);
              }}
            >
              Set Limits
            </Button>
          )}
        </div>
        {!zeroConfirmOpen ? (
          <Button
            type="button"
            variant="destructive"
            size="sm"
            disabled={!canSetZero}
            title="Position joint at mechanical zero first. Briefly enables for firmware SetZero, then disables."
            onClick={() => {
              setZeroError(null);
              setZeroOk(false);
              setZeroConfirmOpen(true);
            }}
          >
            Set Zero
          </Button>
        ) : null}
      </div>
    </section>
  );
}

function MetricCell({
  label,
  value,
  live,
}: {
  label: string;
  value: string;
  live?: boolean;
}) {
  return (
    <div
      className={
        live
          ? 'rounded-sm border border-accent/40 bg-surface-2 px-2 py-1.5'
          : 'rounded-sm border border-line bg-surface-2 px-2 py-1.5'
      }
    >
      <div className="font-mono text-[10px] uppercase tracking-[0.14em] text-muted-foreground">
        {label}
      </div>
      <div className="tabular-nums text-foreground">{value}</div>
    </div>
  );
}
