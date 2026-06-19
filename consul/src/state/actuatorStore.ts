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

interface ActuatorStore {
  sessionId: string | null;
  setSessionId: (sessionId: string | null) => void;

  limitSnapshot: ActuatorLimitSnapshot | null;
  limitsUpdatedAt: number | null;
  setLimitSnapshot: (snapshot: ActuatorLimitSnapshot | null) => void;

  limitsError: string | null;
  setLimitsError: (message: string | null) => void;
}

export const useActuatorStore = create<ActuatorStore>((set) => ({
  sessionId: null,
  setSessionId: (sessionId) => set({ sessionId }),

  limitSnapshot: null,
  limitsUpdatedAt: null,
  setLimitSnapshot: (limitSnapshot) =>
    set({
      limitSnapshot,
      limitsUpdatedAt: limitSnapshot ? Date.now() : null,
      limitsError: null,
    }),

  limitsError: null,
  setLimitsError: (limitsError) => set({ limitsError }),
}));

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

export function resolveJointLimits(
  jointName: string,
  snapshot: ActuatorLimitSnapshot | null,
): StaticJointLimits | null {
  const live = findSnapshotLimit(snapshot, jointName);
  if (live) {
    return {
      kpMax: live.kpMax,
      kdMax: live.kdMax,
      velocityMaxRadS: live.velocityMaxRadS,
      tauFfMaxNm: live.tauFfMaxNm,
    };
  }
  return staticLimitsForJoint(jointName);
}

export function kpMaxForJoint(
  jointName: string,
  snapshot: ActuatorLimitSnapshot | null,
): number | null {
  return resolveJointLimits(jointName, snapshot)?.kpMax ?? null;
}

export function kdMaxForJoint(
  jointName: string,
  snapshot: ActuatorLimitSnapshot | null,
): number | null {
  return resolveJointLimits(jointName, snapshot)?.kdMax ?? null;
}
