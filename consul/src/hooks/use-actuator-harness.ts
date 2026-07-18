import { useEffect } from 'react';

import { fetchActuatorLimits } from '@/lib/gateway-api';
import { isChappeLive } from '@/lib/chappe-config';
import { ensureClientId, useActuatorStore } from '@/state/actuatorStore';

const LIMITS_POLL_MS = 2000;

export function useActuatorHarnessBootstrap(): void {
  const setLimitSnapshot = useActuatorStore((s) => s.setLimitSnapshot);
  const setLimitsError = useActuatorStore((s) => s.setLimitsError);

  useEffect(() => {
    if (!isChappeLive()) {
      return;
    }

    ensureClientId();
    let cancelled = false;

    const pollLimits = () => {
      void fetchActuatorLimits()
        .then((snapshot) => {
          if (!cancelled) {
            setLimitSnapshot(snapshot);
          }
        })
        .catch((error: unknown) => {
          if (!cancelled) {
            setLimitsError(error instanceof Error ? error.message : 'limits fetch failed');
          }
        });
    };

    pollLimits();
    const timer = window.setInterval(pollLimits, LIMITS_POLL_MS);

    return () => {
      cancelled = true;
      window.clearInterval(timer);
    };
  }, [setLimitSnapshot, setLimitsError]);
}
