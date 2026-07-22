import { create } from 'zustand';

export type RestartReason = 'structural' | 'wiring';

export type PendingRestart = {
  profile: string;
  joint: string;
  reason: RestartReason;
  expected_revision: string;
};

type NeedsRestartState = {
  pending: PendingRestart[];
  restartDialogOpen: boolean;
  /** Dialog opened for structural/wiring membership — not Set Limits. */
  dialogReason: RestartReason | null;
  markNeedsRestart: (entry: PendingRestart) => void;
  /** @deprecated Prefer markNeedsRestart with reason. */
  markJointNeedsRestart: (joint: string) => void;
  clearNeedsRestart: () => void;
  openRestartDialog: (opts?: { reason?: RestartReason }) => void;
  closeRestartDialog: () => void;
  isJointPending: (joint: string) => boolean;
};

export const useNeedsRestartStore = create<NeedsRestartState>((set, get) => ({
  pending: [],
  restartDialogOpen: false,
  dialogReason: null,

  markNeedsRestart: (entry) => {
    const joint = entry.joint.trim();
    if (!joint) {
      return;
    }
    set((state) => {
      if (
        state.pending.some(
          (p) =>
            p.joint === joint &&
            p.profile === entry.profile &&
            p.reason === entry.reason,
        )
      ) {
        return state;
      }
      return {
        pending: [
          ...state.pending,
          {
            ...entry,
            joint,
          },
        ],
      };
    });
  },

  markJointNeedsRestart: (joint) => {
    get().markNeedsRestart({
      profile: 'active',
      joint,
      reason: 'wiring',
      expected_revision: '',
    });
  },

  clearNeedsRestart: () => {
    set({ pending: [], restartDialogOpen: false, dialogReason: null });
  },

  openRestartDialog: (opts) => {
    set({
      restartDialogOpen: true,
      dialogReason: opts?.reason ?? 'structural',
    });
  },

  closeRestartDialog: () => {
    set({ restartDialogOpen: false, dialogReason: null });
  },

  isJointPending: (joint) =>
    get().pending.some((p) => p.joint === joint.trim()),
}));

export function selectNeedsRestart(state: NeedsRestartState): boolean {
  return state.pending.length > 0;
}

export function selectPendingJoints(state: NeedsRestartState): string[] {
  return state.pending.map((p) => p.joint);
}
