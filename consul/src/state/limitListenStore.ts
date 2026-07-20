import { create } from 'zustand';

import {
  emptyBounds,
  foldPosition,
  proposedRangeFromBounds,
  type RunningBounds,
} from '@/lib/limit-listen';

export type LimitListenPhase = 'idle' | 'listening' | 'review';

interface LimitListenState {
  jointName: string | null;
  phase: LimitListenPhase;
  bounds: RunningBounds;
  proposedRange: string | null;
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
  proposedRange: null as string | null,
  error: null as string | null,
};

export const useLimitListenStore = create<LimitListenState>((set, get) => ({
  ...initial,

  start: (jointName) =>
    set({
      jointName,
      phase: 'listening',
      bounds: emptyBounds(),
      proposedRange: null,
      error: null,
    }),

  ingestPosition: (jointName, position) => {
    const state = get();
    if (state.phase !== 'listening' || state.jointName !== jointName) {
      return;
    }
    const bounds = foldPosition(state.bounds, position);
    set({ bounds });
  },

  stop: () => {
    const state = get();
    if (state.phase !== 'listening') {
      return;
    }
    const proposed = proposedRangeFromBounds(state.bounds);
    if (!proposed) {
      set({
        phase: 'idle',
        proposedRange: null,
        error: 'Need motion across both limits before stopping.',
      });
      return;
    }
    set({
      phase: 'review',
      proposedRange: proposed,
      error: null,
    });
  },

  abort: (error) =>
    set({
      phase: 'idle',
      proposedRange: null,
      error,
    }),

  discard: () =>
    set({
      phase: 'idle',
      bounds: emptyBounds(),
      proposedRange: null,
      error: null,
    }),

  reset: () => set({ ...initial, bounds: emptyBounds() }),
}));
