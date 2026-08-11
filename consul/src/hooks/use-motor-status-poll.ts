import { useEffect } from 'react';

import { isChappeLive } from '@/lib/chappe-config';
import { postMotorStatusPoll } from '@/lib/gateway-api';

/**
 * Hardware-page solicit interval.
 * Keep above the gateway StatusPoll refill (0.5/s) so timer jitter / remount
 * does not sit on the burst edge; Davout no-ops motors already under type-24.
 */
export const MOTOR_STATUS_POLL_MS = 2500;

/**
 * While enabled, POST a light motor status poll on an interval.
 * Pi re-TX Disable (type-4) per motor that does not already desire Active Reporting;
 * motors reply OperationStatus. Does not enable continuous type-24.
 */
export function useMotorStatusPoll(options: { enabled: boolean }): void {
  const { enabled } = options;

  useEffect(() => {
    if (!enabled || !isChappeLive()) {
      return;
    }

    let cancelled = false;
    let timer: number | undefined;

    const schedule = (delayMs: number) => {
      timer = window.setTimeout(() => {
        void tick();
      }, delayMs);
    };

    const tick = async () => {
      if (cancelled) {
        return;
      }
      try {
        await postMotorStatusPoll();
        schedule(MOTOR_STATUS_POLL_MS);
      } catch (err) {
        // Back off on 429 / transient errors instead of silent tight retries.
        const message = err instanceof Error ? err.message : String(err);
        const rateLimited = message.includes('429');
        schedule(rateLimited ? MOTOR_STATUS_POLL_MS * 2 : MOTOR_STATUS_POLL_MS);
      }
    };

    void tick();

    return () => {
      cancelled = true;
      if (timer !== undefined) {
        window.clearTimeout(timer);
      }
    };
  }, [enabled]);
}
