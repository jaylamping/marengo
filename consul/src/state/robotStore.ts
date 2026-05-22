import { create } from 'zustand';

/**
 * Global robot state (Zustand).
 * This will grow to hold the full RobotState from protobuf,
 * joint positions, safety state, active gains, etc.
 */
interface RobotState {
  jointPosition: number; // demo value for the placeholder 3D torus
  setJointPosition: (v: number) => void;

  // Future real fields (examples):
  // connected: boolean;
  // safetyState: SafetyState;
  // joints: Record<string, JointState>;
}

export const useRobotStore = create<RobotState>((set) => ({
  jointPosition: 0.4,
  setJointPosition: (v) => set({ jointPosition: v }),
}));