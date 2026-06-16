import { create } from 'zustand';

import type { RobotState, SafetyState, ImuSample } from '@/gen/marengo/v1/marengo_pb';
import type { JointTrackingPoint } from '@/components/dashboard/charts/types';
import { dummyShoulderPitchTracking } from '@/components/dashboard/charts/constants';
import { isChappeLive } from '@/lib/chappe-config';

export type OperationalModeLabel = 'DISABLED' | 'READY' | 'ACTIVE' | null;

export type JointTrackingPointByJoint = Record<string, JointTrackingPoint[]>;

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

  trackingPoints: JointTrackingPoint[]; // legacy alias or empty
  trackingPointsByJoint: JointTrackingPointByJoint;
  appendTrackingPoint: (jointName: string, point: JointTrackingPoint) => void;
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

  trackingPoints: isChappeLive() ? [] : dummyShoulderPitchTracking.points, // keep legacy shape for backwards compat if needed
  trackingPointsByJoint: {},
  appendTrackingPoint: (jointName, point) =>
    set((state) => {
      const current = state.trackingPointsByJoint[jointName] || [];
      const next = [...current, point];
      if (next.length > 120) {
        next.splice(0, next.length - 120);
      }
      return {
        trackingPointsByJoint: {
          ...state.trackingPointsByJoint,
          [jointName]: next,
        },
      };
    }),
}));
