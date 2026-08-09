import { create as createZustand } from 'zustand';

/**
 * Legacy session helper formerly used for browser "Ready" / Home unlock.
 * Commissioning Reference Ready is wire-only (`JointState.homing_state == Verified`).
 * Marks are no-ops — do not persist or fabricate readiness.
 */
interface ActuatorZeroStore {
  /** Always empty — retained for call-site compatibility until Testing cleanup. */
  zeroed: Record<string, true>;
  markZeroed: (joint: string) => void;
  markAllZeroed: (joints: string[]) => void;
  isZeroed: (joint: string) => boolean;
  /** Test helper. */
  reset: () => void;
}

export const useActuatorZeroStore = createZustand<ActuatorZeroStore>((set) => ({
  zeroed: {},
  markZeroed: (_joint) => {
    // no-op: Reference Ready comes from Chappe wire only
  },
  markAllZeroed: (_joints) => {
    // no-op
  },
  isZeroed: () => false,
  reset: () => {
    set({ zeroed: {} });
  },
}));
