import { create } from 'zustand';

import type {
  ActuatorLimitSnapshot,
  JointActuatorLimit,
} from '@/gen/marengo/v1/marengo_pb';
import {
  staticLimitsForJoint,
  toCanonicalBenchJoint,
  type StaticJointLimits,
} from '@/data/actuator-joints';

export type ActuatorBootstrapStatus =
  | { kind: 'idle' }
  | { kind: 'ready'; clientId: string }
  | { kind: 'limitsError'; clientId: string; message: string };

interface ActuatorStore {
  bootstrap: ActuatorBootstrapStatus;
  setBootstrapReady: (clientId: string) => void;
  setLimitsError: (message: string) => void;

  limitSnapshot: ActuatorLimitSnapshot | null;
  limitsUpdatedAt: number | null;
  setLimitSnapshot: (snapshot: ActuatorLimitSnapshot | null) => void;

  lastError: string | null;
  setLastError: (message: string | null) => void;

  commandSeq: bigint;
  nextCommandSeq: () => bigint;
}

function mintClientId(): string {
  if (typeof crypto !== 'undefined' && typeof crypto.randomUUID === 'function') {
    return crypto.randomUUID();
  }
  return `consul-${Date.now()}-${Math.random().toString(16).slice(2)}`;
}

export const useActuatorStore = create<ActuatorStore>((set, get) => ({
  bootstrap: { kind: 'idle' },
  setBootstrapReady: (clientId) => set({ bootstrap: { kind: 'ready', clientId } }),
  setLimitsError: (message) => {
    const clientId =
      get().bootstrap.kind === 'idle' ? mintClientId() : getClientId(get().bootstrap);
    set({ bootstrap: { kind: 'limitsError', clientId, message } });
  },

  limitSnapshot: null,
  limitsUpdatedAt: null,
  setLimitSnapshot: (limitSnapshot) => {
    const clientId =
      get().bootstrap.kind === 'idle' ? mintClientId() : getClientId(get().bootstrap);
    set({
      limitSnapshot,
      limitsUpdatedAt: limitSnapshot ? Date.now() : null,
      bootstrap: { kind: 'ready', clientId },
    });
  },

  lastError: null,
  setLastError: (lastError) => set({ lastError }),

  commandSeq: 0n,
  nextCommandSeq: () => {
    const next = get().commandSeq + 1n;
    set({ commandSeq: next });
    return next;
  },
}));

function getClientId(status: ActuatorBootstrapStatus): string {
  switch (status.kind) {
    case 'idle':
      return mintClientId();
    case 'ready':
    case 'limitsError':
      return status.clientId;
    default: {
      const _exhaustive: never = status;
      return _exhaustive;
    }
  }
}

/** Client id used as OperatorCommand.session_id for rate-limit keys. */
export function selectClientId(status: ActuatorBootstrapStatus): string | null {
  switch (status.kind) {
    case 'idle':
      return null;
    case 'ready':
    case 'limitsError':
      return status.clientId;
    default: {
      const _exhaustive: never = status;
      return _exhaustive;
    }
  }
}

export function ensureClientId(): string {
  const current = useActuatorStore.getState().bootstrap;
  const existing = selectClientId(current);
  if (existing) {
    return existing;
  }
  const clientId = mintClientId();
  useActuatorStore.getState().setBootstrapReady(clientId);
  return clientId;
}

export function findSnapshotLimit(
  snapshot: ActuatorLimitSnapshot | null,
  jointName: string,
): JointActuatorLimit | null {
  const canonical = toCanonicalBenchJoint(jointName);
  if (!canonical || !snapshot?.joints.length) {
    return null;
  }
  return snapshot.joints.find((entry) => entry.joint === canonical) ?? null;
}

/** Live Davout position envelope (hard = enable/fault SoT; soft = ADR 0009 inset). */
export type LiveJointEnvelope = {
  hardLowerRad: number;
  hardUpperRad: number;
  softLowerRad: number;
  softUpperRad: number;
};

/** Live snapshot caps only — never fall back to static display limits for commands. */
export function liveJointLimits(
  jointName: string,
  snapshot: ActuatorLimitSnapshot | null,
): StaticJointLimits | null {
  const live = findSnapshotLimit(snapshot, jointName);
  if (!live) {
    return null;
  }
  return {
    kpMax: live.kpMax,
    kdMax: live.kdMax,
    velocityMaxRadS: live.velocityMaxRadS,
    tauFfMaxNm: live.tauFfMaxNm,
  };
}

/**
 * Davout effective position envelope from ActuatorLimitSnapshot.
 * Hard is URDF ∩ motors.yaml bench — the only range UI/enable should trust.
 */
export function liveJointEnvelope(
  jointName: string,
  snapshot: ActuatorLimitSnapshot | null,
): LiveJointEnvelope | null {
  const live = findSnapshotLimit(snapshot, jointName);
  if (!live) {
    return null;
  }
  const hardLowerRad = live.posLowerRad;
  const hardUpperRad = live.posUpperRad;
  if (
    !Number.isFinite(hardLowerRad) ||
    !Number.isFinite(hardUpperRad) ||
    hardLowerRad >= hardUpperRad
  ) {
    return null;
  }
  const softLowerRad = Number.isFinite(live.posSoftLowerRad)
    ? live.posSoftLowerRad
    : hardLowerRad;
  const softUpperRad = Number.isFinite(live.posSoftUpperRad)
    ? live.posSoftUpperRad
    : hardUpperRad;
  return {
    hardLowerRad,
    hardUpperRad,
    softLowerRad,
    softUpperRad,
  };
}

/** Display helper: live caps preferred, static reference only when snapshot missing. */
export function resolveJointLimits(
  jointName: string,
  snapshot: ActuatorLimitSnapshot | null,
): StaticJointLimits | null {
  return liveJointLimits(jointName, snapshot) ?? staticLimitsForJoint(jointName);
}

export function jointLimitMax(
  jointName: string,
  snapshot: ActuatorLimitSnapshot | null,
  param: 'kp' | 'kd',
): number | null {
  const limits = liveJointLimits(jointName, snapshot);
  if (!limits) {
    return null;
  }
  return param === 'kp' ? limits.kpMax : limits.kdMax;
}
