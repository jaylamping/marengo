import { create } from 'zustand';

type NeedsRestartState = {
  pendingRestartJoints: string[];
  restartDialogOpen: boolean;
  /** Richer copy when opened immediately after Set Limits Apply. */
  dialogFromApply: boolean;
  markJointNeedsRestart: (joint: string) => void;
  clearNeedsRestart: () => void;
  openRestartDialog: (opts?: { fromApply?: boolean }) => void;
  closeRestartDialog: () => void;
  isJointPending: (joint: string) => boolean;
};

export const useNeedsRestartStore = create<NeedsRestartState>((set, get) => ({
  pendingRestartJoints: [],
  restartDialogOpen: false,
  dialogFromApply: false,

  markJointNeedsRestart: (joint) => {
    const name = joint.trim();
    if (!name) {
      return;
    }
    set((state) => {
      if (state.pendingRestartJoints.includes(name)) {
        return state;
      }
      return {
        pendingRestartJoints: [...state.pendingRestartJoints, name],
      };
    });
  },

  clearNeedsRestart: () => {
    set({ pendingRestartJoints: [], restartDialogOpen: false, dialogFromApply: false });
  },

  openRestartDialog: (opts) => {
    set({
      restartDialogOpen: true,
      dialogFromApply: opts?.fromApply === true,
    });
  },

  closeRestartDialog: () => {
    set({ restartDialogOpen: false, dialogFromApply: false });
  },

  isJointPending: (joint) => get().pendingRestartJoints.includes(joint.trim()),
}));

export function selectNeedsRestart(state: NeedsRestartState): boolean {
  return state.pendingRestartJoints.length > 0;
}
