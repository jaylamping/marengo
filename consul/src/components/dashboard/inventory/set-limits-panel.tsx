import { useEffect, useEffectEvent, useId, useRef, useState } from 'react';

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
import { persistJointLimits } from '@/lib/persist-joint-limits';
import { queryClient } from '@/lib/query-client';
import { queryKeys } from '@/lib/query-keys';
import { subscribeTeachSamples } from '@/lib/teach-sample-bus';
import { useLimitListenStore } from '@/state/limitListenStore';
import { useRobotStore } from '@/state/robotStore';

const SET_LIMITS_HELP =
  'Motors must be disabled (not ACTIVE) for Set Limits. Support the assembly, then sweep the joint to both hard stops while Consul samples position. Stop to propose min/max, then Apply writes motors.yaml bench limits via the gateway (restart marengo-pi to load hard limits into Davout). Set Zero briefly enables for firmware zero at the current pose, then disables again.';

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
  const proposal = useLimitListenStore((s) => s.proposal);
  const error = useLimitListenStore((s) => s.error);
  const start = useLimitListenStore((s) => s.start);
  const ingestPosition = useLimitListenStore((s) => s.ingestPosition);
  const stop = useLimitListenStore((s) => s.stop);
  const abort = useLimitListenStore((s) => s.abort);
  const discard = useLimitListenStore((s) => s.discard);
  const reset = useLimitListenStore((s) => s.reset);
  const [zeroBusy, setZeroBusy] = useState(false);
  const [zeroError, setZeroError] = useState<string | null>(null);
  const [zeroOk, setZeroOk] = useState(false);
  const [zeroConfirmOpen, setZeroConfirmOpen] = useState(false);
  const [signTestPassed, setSignTestPassed] = useState(false);
  const [applyBusy, setApplyBusy] = useState(false);
  const [applyError, setApplyError] = useState<string | null>(null);
  const [applyOk, setApplyOk] = useState<string | null>(null);
  const applyInFlightRef = useRef(false);
  const proposedRange = proposal?.display ?? null;

  const confirmTitleId = useId();
  const confirmDescId = useId();

  const isThisSession = activeJoint === jointName;
  const listening = phase === 'listening' && isThisSession;
  const reviewing = phase === 'review' && isThisSession;

  const gate = { connected, operationalMode };
  const canStart = canStartLimitListen(gate);
  const blockReason = limitListenBlockReason(gate);
  // Never Set Zero while ACTIVE — Pi refuses and would otherwise risk dropping hold.
  const canSetZero =
    connected &&
    operationalMode !== null &&
    operationalMode !== 'ACTIVE' &&
    !listening &&
    !zeroBusy &&
    !reviewing;
  const setZeroBlockReason =
    connected && operationalMode === 'ACTIVE'
      ? 'Disable motors first — Set Zero refused while ACTIVE.'
      : connected && operationalMode === null
        ? 'Waiting for operational mode…'
        : null;

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
    if (!applyOk) {
      return;
    }
    const timer = window.setTimeout(() => setApplyOk(null), 6000);
    return () => window.clearTimeout(timer);
  }, [applyOk]);

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
    // Re-check before post — dialog can stay open across DISABLED→ACTIVE.
    if (
      !connected ||
      operationalMode === null ||
      operationalMode === 'ACTIVE' ||
      !signTestPassed
    ) {
      setZeroError(
        operationalMode === 'ACTIVE'
          ? 'Set Zero refused while ACTIVE — disable motors first.'
          : !signTestPassed
            ? 'Confirm sign/direction at mechanical home before Set Zero.'
            : 'Waiting for operational mode…',
      );
      return;
    }
    setZeroBusy(true);
    setZeroError(null);
    setZeroOk(false);
    try {
      await postSetZeroCommand(jointName, { signTestPassed: true });
      // Gateway 200 only means queued on Chappe — do not bump teach calibration yet.
      setZeroOk(true);
      setZeroConfirmOpen(false);
      setSignTestPassed(false);
    } catch (e) {
      setZeroError(e instanceof Error ? e.message : 'Set Zero failed');
    } finally {
      setZeroBusy(false);
    }
  };

  const runApplyLimits = async () => {
    if (!proposal || applyInFlightRef.current) {
      return;
    }
    applyInFlightRef.current = true;
    setApplyBusy(true);
    setApplyError(null);
    setApplyOk(null);
    try {
      const result = await persistJointLimits(jointName, {
        lower: proposal.lower,
        upper: proposal.upper,
      });
      if (!result.ok) {
        setApplyError(result.message);
        return;
      }
      // Draft only — do not write inventoryOverridesStore; config snapshot is SoT.
      onApplyRange(proposal.display);
      discard();
      setApplyOk(
        result.restartRequired
          ? `${result.message} Restart marengo-pi to load new hard limits.`
          : result.message,
      );
      try {
        await queryClient.invalidateQueries({
          queryKey: queryKeys.configSnapshot,
        });
      } catch {
        // YAML already written; a stale cache is recoverable on next refresh.
      }
    } catch (e) {
      setApplyError(e instanceof Error ? e.message : 'Limits persist failed');
    } finally {
      applyInFlightRef.current = false;
      setApplyBusy(false);
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

      {setZeroBlockReason && !listening && !reviewing && !zeroConfirmOpen ? (
        <p className="text-xs text-muted-foreground" role="status">
          {setZeroBlockReason}
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

      {applyError ? (
        <p className="text-xs text-fault" role="status">
          {applyError}
        </p>
      ) : null}
      {applyOk ? (
        <p className="text-xs text-ok" role="status">
          {applyOk}
        </p>
      ) : null}

      {zeroError ? (
        <p className="text-xs text-fault" role="status">
          {zeroError}
        </p>
      ) : null}
      {zeroOk ? (
        <p className="text-xs text-ok" role="status">
          Set Zero queued — watch telemetry for pos near 0 and Disabled before
          Set Limits. Calibration epoch is not bumped until zero is verified.
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
            <label className="flex items-start gap-2 text-xs text-foreground">
              <input
                type="checkbox"
                className="mt-0.5"
                checked={signTestPassed}
                disabled={zeroBusy}
                onChange={(e) => setSignTestPassed(e.target.checked)}
              />
              <span>
                Sign/direction checked at mechanical home (required attestation).
              </span>
            </label>
          </div>
          <div className="flex flex-wrap justify-end gap-2">
            <Button
              type="button"
              variant="outline"
              size="sm"
              disabled={zeroBusy}
              autoFocus
              onClick={() => {
                setZeroConfirmOpen(false);
                setSignTestPassed(false);
              }}
            >
              Cancel
            </Button>
            <Button
              type="button"
              variant="destructive"
              size="sm"
              disabled={zeroBusy || !signTestPassed}
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
                disabled={applyBusy || !proposedRange}
                onClick={() => {
                  void runApplyLimits();
                }}
              >
                {applyBusy ? 'Saving…' : 'Apply Limits'}
              </Button>
              <Button
                type="button"
                variant="outline"
                size="sm"
                disabled={applyBusy}
                onClick={() => {
                  setApplyError(null);
                  setApplyOk(null);
                  discard();
                }}
              >
                Discard
              </Button>
            </>
          ) : (
            <Button
              type="button"
              size="sm"
              disabled={!canStart || zeroConfirmOpen || zeroBusy || zeroOk}
              onClick={() => {
                setZeroOk(false);
                setZeroError(null);
                setApplyOk(null);
                setApplyError(null);
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
            title={
              setZeroBlockReason ??
              'Position joint at mechanical zero first. Briefly enables for firmware SetZero, then disables.'
            }
            onClick={() => {
              setZeroError(null);
              setZeroOk(false);
              setSignTestPassed(false);
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
