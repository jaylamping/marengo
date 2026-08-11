import { useEffect, useRef, useState } from 'react';

import { isChappeLive } from '@/lib/chappe-config';
import { postActiveReportingLease } from '@/lib/gateway-api';
import { ensureClientId } from '@/state/actuatorStore';

const RENEW_MS = 10_000;

export type ActiveReportingLeaseUiState = 'idle' | 'requested' | 'failed';

function mintLeaseId(): string {
  if (typeof crypto !== 'undefined' && typeof crypto.randomUUID === 'function') {
    return crypto.randomUUID();
  }
  return `lease-${Date.now()}-${Math.random().toString(16).slice(2)}`;
}

/**
 * Holds a per-joint Active Reporting (type-24) lease while `enabled`.
 * HTTP 200 means publish ACK only — UI shows Enhanced logging, not wire confirmation.
 */
export function useActiveReportingLease(options: {
  joint: string | null;
  enabled: boolean;
}): ActiveReportingLeaseUiState {
  const { joint, enabled } = options;
  const [uiState, setUiState] = useState<ActiveReportingLeaseUiState>('idle');
  const leaseRef = useRef<{ joint: string; leaseId: string; clientId: string } | null>(
    null,
  );

  useEffect(() => {
    if (!enabled || !joint || !isChappeLive()) {
      setUiState('idle');
      return;
    }

    let cancelled = false;
    const clientId = ensureClientId();
    const leaseId = mintLeaseId();
    leaseRef.current = { joint, leaseId, clientId };
    setUiState('idle');

    const release = (keepalive: boolean) => {
      const held = leaseRef.current;
      if (!held) {
        return;
      }
      leaseRef.current = null;
      void postActiveReportingLease({
        joint: held.joint,
        clientId: held.clientId,
        leaseId: held.leaseId,
        action: 'release',
        keepalive,
      }).catch(() => {
        // Best-effort; TTL expiry is the backstop.
      });
    };

    const onPageHide = () => {
      release(true);
    };
    window.addEventListener('pagehide', onPageHide);

    void postActiveReportingLease({
      joint,
      clientId,
      leaseId,
      action: 'acquire',
    })
      .then(() => {
        if (cancelled) {
          return;
        }
        setUiState('requested');
      })
      .catch(() => {
        if (!cancelled) {
          setUiState('failed');
        }
      });

    const renewTimer = window.setInterval(() => {
      const held = leaseRef.current;
      if (!held || cancelled) {
        return;
      }
      void postActiveReportingLease({
        joint: held.joint,
        clientId: held.clientId,
        leaseId: held.leaseId,
        action: 'renew',
      })
        .then(() => {
          if (!cancelled) {
            setUiState('requested');
          }
        })
        .catch(() => {
          if (!cancelled) {
            setUiState('failed');
          }
        });
    }, RENEW_MS);

    return () => {
      cancelled = true;
      window.clearInterval(renewTimer);
      window.removeEventListener('pagehide', onPageHide);
      release(false);
      setUiState('idle');
    };
  }, [enabled, joint]);

  return uiState;
}
