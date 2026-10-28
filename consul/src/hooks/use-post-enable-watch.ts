import { useCallback, useEffect, useRef, useState } from 'react';

import type { ActuatorLimitSnapshot } from '@/gen/marengo/v1/marengo_pb';
import type { ConfigSnapshotDto } from '@/lib/config-api';
import { interpretPostEnableWatch } from '@/lib/enable-feedback';
import { useRobotStore } from '@/state/robotStore';

const WATCH_TICK_MS = 200;
/** The Pi's refusal text for an Enable (lease, degraded hold, preflight). */
const ENABLE_REFUSED = /\benable refused\b/i;
/** The refusal can arrive before the Enable POST's response does. */
const REFUSAL_LOOKBACK_MS = 1000;

export type PostEnableOutcome = {
  kind: 'ok' | 'error' | 'pending';
  message: string;
};

/**
 * After an Enable POST is accepted, watch telemetry until the outcome is clear:
 * ACTIVE held, a safety trip or fault, a published `enable refused`, a drop
 * back to DISABLED after ACTIVE, or no ACTIVE within the Pi's bound (UI-07).
 * A gateway 200 only means the request was published.
 */
export function usePostEnableWatch(
  config: ConfigSnapshotDto | null,
  limitSnapshot: ActuatorLimitSnapshot | null,
) {
  const [startedAtMs, setStartedAtMs] = useState<number | null>(null);
  const [outcome, setOutcome] = useState<PostEnableOutcome | null>(null);
  const contextRef = useRef({ config, limitSnapshot });
  contextRef.current = { config, limitSnapshot };
  // When ACTIVE was first seen after this Enable; null until then.
  const firstActiveAtMsRef = useRef<number | null>(null);

  useEffect(() => {
    if (startedAtMs === null) {
      return;
    }
    firstActiveAtMsRef.current = null;
    const tick = () => {
      const robot = useRobotStore.getState();
      const nowMs = Date.now();
      if (robot.operationalMode === 'ACTIVE' && firstActiveAtMsRef.current === null) {
        firstActiveAtMsRef.current = nowMs;
      }
      const refusal = robot.lastActionRefusal;
      const enableRefusal =
        refusal &&
        refusal.receivedAtMs >= startedAtMs - REFUSAL_LOOKBACK_MS &&
        ENABLE_REFUSED.test(refusal.reason)
          ? refusal.reason
          : null;
      const result = interpretPostEnableWatch({
        elapsedMs: nowMs - startedAtMs,
        operationalMode: robot.operationalMode,
        safetyState: robot.safetyState,
        robotState: robot.robotState,
        config: contextRef.current.config,
        limitSnapshot: contextRef.current.limitSnapshot,
        activeForMs:
          firstActiveAtMsRef.current === null ? null : nowMs - firstActiveAtMsRef.current,
        enableRefusal,
      });
      setOutcome(result.message ? { kind: result.kind, message: result.message } : null);
      if (result.done) {
        setStartedAtMs(null);
      }
    };
    const intervalId = window.setInterval(tick, WATCH_TICK_MS);
    return () => window.clearInterval(intervalId);
  }, [startedAtMs]);

  const start = useCallback(() => {
    setOutcome({ kind: 'pending', message: 'Enable queued — waiting for ACTIVE…' });
    setStartedAtMs(Date.now());
  }, []);

  const clear = useCallback(() => {
    setStartedAtMs(null);
    setOutcome(null);
  }, []);

  return { outcome, start, clear };
}
