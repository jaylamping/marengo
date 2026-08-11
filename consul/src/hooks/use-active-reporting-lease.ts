import { useEffect, useRef, useState } from 'react';

import { isChappeLive } from '@/lib/chappe-config';
import { postActiveReportingLease } from '@/lib/gateway-api';
import { ensureClientId } from '@/state/actuatorStore';

const RENEW_MS = 10_000;

export type ActiveReportingLeaseUiState = 'idle' | 'requested' | 'failed';

type HeldLease = {
  joint: string;
  leaseId: string;
  clientId: string;
};

function mintLeaseId(): string {
  if (typeof crypto !== 'undefined' && typeof crypto.randomUUID === 'function') {
    return crypto.randomUUID();
  }
  return `lease-${Date.now()}-${Math.random().toString(16).slice(2)}`;
}

function stableJointKey(joints: readonly string[]): string {
  return [...joints].filter(Boolean).sort().join('\0');
}

/**
 * Holds Active Reporting (type-24) leases for many joints while `enabled`.
 * HTTP 200 means publish ACK only — not confirmed wire reporting.
 */
export function useActiveReportingLeases(options: {
  joints: readonly string[];
  enabled: boolean;
}): ActiveReportingLeaseUiState {
  const { joints, enabled } = options;
  const jointKey = stableJointKey(joints);
  const [uiState, setUiState] = useState<ActiveReportingLeaseUiState>('idle');
  const leasesRef = useRef<HeldLease[]>([]);

  useEffect(() => {
    const jointList = jointKey.length > 0 ? jointKey.split('\0') : [];
    if (!enabled || jointList.length === 0 || !isChappeLive()) {
      setUiState('idle');
      return;
    }

    let cancelled = false;
    const clientId = ensureClientId();
    const held: HeldLease[] = jointList.map((joint) => ({
      joint,
      leaseId: mintLeaseId(),
      clientId,
    }));
    leasesRef.current = held;
    setUiState('idle');

    const releaseAll = (keepalive: boolean) => {
      const current = leasesRef.current;
      leasesRef.current = [];
      for (const lease of current) {
        void postActiveReportingLease({
          joint: lease.joint,
          clientId: lease.clientId,
          leaseId: lease.leaseId,
          action: 'release',
          keepalive,
        }).catch(() => {
          // Best-effort; TTL expiry is the backstop.
        });
      }
    };

    const onPageHide = () => {
      releaseAll(true);
    };
    window.addEventListener('pagehide', onPageHide);

    void Promise.all(
      held.map((lease) =>
        postActiveReportingLease({
          joint: lease.joint,
          clientId: lease.clientId,
          leaseId: lease.leaseId,
          action: 'acquire',
        }),
      ),
    )
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

    const renewTimer = window.setInterval(() => {
      const current = leasesRef.current;
      if (current.length === 0 || cancelled) {
        return;
      }
      void Promise.all(
        current.map((lease) =>
          postActiveReportingLease({
            joint: lease.joint,
            clientId: lease.clientId,
            leaseId: lease.leaseId,
            action: 'renew',
          }),
        ),
      )
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
      releaseAll(false);
      setUiState('idle');
    };
  }, [enabled, jointKey]);

  return uiState;
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
  return useActiveReportingLeases({
    joints: joint ? [joint] : [],
    enabled: Boolean(enabled && joint),
  });
}
