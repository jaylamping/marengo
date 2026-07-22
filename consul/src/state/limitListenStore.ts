import { create } from 'zustand';

import {
  emptyBounds,
  foldPosition,
  proposedLimitsFromBounds,
  type ProposedJointLimits,
  type RunningBounds,
} from '@/lib/limit-listen';

export type LimitListenPhase = 'idle' | 'listening' | 'review';

interface LimitListenState {
  jointName: string | null;
  phase: LimitListenPhase;
  bounds: RunningBounds;
  proposal: ProposedJointLimits | null;
  error: string | null;

  start: (jointName: string) => void;
  ingestPosition: (jointName: string, position: number) => void;
  stop: () => void;
  abort: (error: string) => void;
  discard: () => void;
  reset: () => void;
}

const initial = {
  jointName: null as string | null,
  phase: 'idle' as LimitListenPhase,
  bounds: emptyBounds(),
  proposal: null as ProposedJointLimits | null,
  error: null as string | null,
};

export const useLimitListenStore = create<LimitListenState>((set, get) => ({
  ...initial,

  start: (jointName) =>
    set({
      jointName,
      phase: 'listening',
      bounds: emptyBounds(),
      proposal: null,
      error: null,
    }),

  ingestPosition: (jointName, position) => {
    const state = get();
    if (state.phase !== 'listening' || state.jointName !== jointName) {
      return;
    }
    set({ bounds: foldPosition(state.bounds, position) });
  },

  stop: () => {
    const state = get();
    if (state.phase !== 'listening') {
      return;
    }
    const proposal = proposedLimitsFromBounds(state.bounds);
    if (!proposal) {
      set({
        phase: 'idle',
        proposal: null,
        error: 'Need motion across both limits before stopping.',
      });
      return;
    }
    set({
      phase: 'review',
      proposal,
      error: null,
    });
  },

  abort: (error) =>
    set({
      phase: 'idle',
      proposal: null,
      error,
    }),

  discard: () =>
    set({
      phase: 'idle',
      bounds: emptyBounds(),
      proposal: null,
      error: null,
    }),

  reset: () => set({ ...initial, bounds: emptyBounds() }),
}));
