import { useCallback, useEffect, useRef, useState } from 'react';

import type { ActuatorLimitSnapshot } from '@/gen/marengo/v1/marengo_pb';
import type { ConfigSnapshotDto } from '@/lib/config-api';
import { interpretPostEnableWatch } from '@/lib/enable-feedback';
import { useRobotStore } from '@/state/robotStore';

const WATCH_TICK_MS = 200;

export type PostEnableOutcome = {
  kind: 'ok' | 'error' | 'pending';
  message: string;
};

/**
 * After an Enable POST is accepted, watch telemetry until the outcome is clear:
 * ACTIVE held, a safety trip or fault, or a drop back to DISABLED (UI-07).
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

  useEffect(() => {
    if (startedAtMs === null) {
      return;
    }
    const tick = () => {
      const robot = useRobotStore.getState();
      const result = interpretPostEnableWatch({
        elapsedMs: Date.now() - startedAtMs,
        operationalMode: robot.operationalMode,
        safetyState: robot.safetyState,
        robotState: robot.robotState,
        config: contextRef.current.config,
        limitSnapshot: contextRef.current.limitSnapshot,
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
