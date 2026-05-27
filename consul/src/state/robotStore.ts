import { create } from 'zustand';

import type { RobotState, SafetyState, ImuSample } from '@/gen/marengo/v1/marengo_pb';
import type { JointTrackingPoint } from '@/components/dashboard/charts/types';
import { dummyShoulderPitchTracking } from '@/components/dashboard/charts/constants';
import { isChappeLive } from '@/lib/chappe-config';

export type OperationalModeLabel = 'DISABLED' | 'READY' | 'ACTIVE' | null;

interface RobotStore {
  /** Legacy demo slider (3D placeholder). */
  jointPosition: number;
  setJointPosition: (v: number) => void;

  connected: boolean;
  setConnected: (v: boolean) => void;

  operationalMode: OperationalModeLabel;
  setOperationalMode: (mode: OperationalModeLabel) => void;

  robotState: RobotState | null;
  setRobotState: (state: RobotState | null) => void;

  safetyState: SafetyState | null;
  setSafetyState: (state: SafetyState | null) => void;

  imuSample: ImuSample | null;
  setImuSample: (sample: ImuSample | null) => void;

  gatewayError: string | null;
  setGatewayError: (message: string | null) => void;

  trackingPoints: JointTrackingPoint[];
  appendTrackingPoint: (point: JointTrackingPoint) => void;
}

const initialTracking = isChappeLive()
  ? []
  : dummyShoulderPitchTracking.points;

export const useRobotStore = create<RobotStore>((set) => ({
  jointPosition: 0.4,
  setJointPosition: (v) => set({ jointPosition: v }),

  connected: false,
  setConnected: (connected) => set({ connected }),

  operationalMode: null,
  setOperationalMode: (operationalMode) => set({ operationalMode }),

  robotState: null,
  setRobotState: (robotState) => set({ robotState }),

  safetyState: null,
  setSafetyState: (safetyState) => set({ safetyState }),

  imuSample: null,
  setImuSample: (imuSample) => set({ imuSample }),

  gatewayError: null,
  setGatewayError: (gatewayError) => set({ gatewayError }),

  trackingPoints: initialTracking,
  appendTrackingPoint: (point) =>
    set((state) => {
      const next = [...state.trackingPoints, point];
      if (next.length > 120) {
        next.splice(0, next.length - 120);
      }
      return { trackingPoints: next };
    }),
}));
