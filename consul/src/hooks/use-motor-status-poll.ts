import { useEffect } from 'react';

import { isChappeLive } from '@/lib/chappe-config';
import { postMotorStatusPoll } from '@/lib/gateway-api';

/** Hardware-page solicit interval — keep well below bus saturation. */
export const MOTOR_STATUS_POLL_MS = 2000;

/**
 * While enabled, POST a light motor status poll every ~2 s.
 * Pi re-TX Disable (type-4) per loaded motor; motors reply OperationStatus.
 * Does not enable continuous Active Reporting (type-24).
 */
export function useMotorStatusPoll(options: { enabled: boolean }): void {
  const { enabled } = options;

  useEffect(() => {
    if (!enabled || !isChappeLive()) {
      return;
    }

    let cancelled = false;

    const poll = () => {
      if (cancelled) {
        return;
      }
      void postMotorStatusPoll().catch(() => {
        // Best-effort; facets stay Offline/stale until the next successful poll.
      });
    };

    poll();
    const timer = window.setInterval(poll, MOTOR_STATUS_POLL_MS);

    return () => {
      cancelled = true;
      window.clearInterval(timer);
    };
  }, [enabled]);
}
