import type {
  AutoLearnLandmark,
  AutoLearnStage,
} from '@marengo/compound-auto-learn';
import { create as createZustand } from 'zustand';

export type AutoLearnStatus =
  | 'idle'
  | 'calling'
  | 'draft'
  | 'applied'
  | 'error';

export type AutoLearnDraft = {
  presetId: string;
  stage: AutoLearnStage;
  description: string;
  landmarks: AutoLearnLandmark[];
  cadenceScale: number;
  settleDwellSec: number;
  speedMultiplier: number;
};

interface AutoLearnStore {
  stage: AutoLearnStage;
  includeLogs: boolean;
  feedback: string;
  status: AutoLearnStatus;
  error: string | null;
  logAttachNote: string | null;
  draft: AutoLearnDraft | null;
  lastGenerateAtMs: number | null;
  /** Operator ran Test proposal (dry-run or hardware) for this draft. */
  proposalTested: boolean;
  /** One-line review strip hint after test/apply. */
  reviewHint: string | null;
  /** Applied via Auto Learn for Dry Run soft-gate / speed clamp. */
  appliedMeta: Record<
    string,
    { stage: AutoLearnStage; source: 'auto_learn' }
  >;
  requestGeneration: number;

  setStage: (stage: AutoLearnStage) => void;
  setIncludeLogs: (v: boolean) => void;
  setFeedback: (v: string) => void;
  clearFeedback: () => void;
  setStatus: (status: AutoLearnStatus, error?: string | null) => void;
  setLogAttachNote: (note: string | null) => void;
  setReviewHint: (hint: string | null) => void;
  setDraft: (draft: AutoLearnDraft | null) => void;
  setLandmarkIncluded: (id: string, included: boolean) => void;
  markProposalTested: (mode?: 'dry_run' | 'hardware') => void;
  markApplied: (presetId: string, stage: AutoLearnStage) => void;
  clearApplied: (presetId: string) => void;
  bumpGeneration: () => number;
  resetForPreset: () => void;
}

export const useAutoLearnStore = createZustand<AutoLearnStore>((set, get) => ({
  stage: 'crawl',
  includeLogs: false,
  feedback: '',
  status: 'idle',
  error: null,
  logAttachNote: null,
  draft: null,
  lastGenerateAtMs: null,
  proposalTested: false,
  reviewHint: null,
  appliedMeta: {},
  requestGeneration: 0,

  setStage: (stage) => set({ stage }),
  setIncludeLogs: (includeLogs) => set({ includeLogs }),
  setFeedback: (feedback) => set({ feedback }),
  clearFeedback: () => set({ feedback: '' }),
  setStatus: (status, error = null) => set({ status, error }),
  setLogAttachNote: (logAttachNote) => set({ logAttachNote }),
  setReviewHint: (reviewHint) => set({ reviewHint }),
  setDraft: (draft) => {
    const prev = get();
    set({
      draft,
      status: draft ? 'draft' : prev.status === 'draft' ? 'idle' : prev.status,
      lastGenerateAtMs: draft ? Date.now() : prev.lastGenerateAtMs,
      proposalTested: false,
      reviewHint: null,
    });
  },
  setLandmarkIncluded: (id, included) =>
    set((state) => {
      if (!state.draft) return state;
      return {
        draft: {
          ...state.draft,
          landmarks: state.draft.landmarks.map((lm) =>
            lm.id === id ? { ...lm, included } : lm,
          ),
          // Inclusion edits invalidate the last Dry Run.
        },
        proposalTested: false,
        reviewHint: null,
      };
    }),
  markProposalTested: (mode = 'dry_run') =>
    set({
      proposalTested: true,
      reviewHint:
        mode === 'hardware'
          ? 'Live hardware test started — support the arm, watch Playback. Apply when it looks right.'
          : 'Dry Run started — watch Playback progress. Apply when it looks right.',
    }),
  markApplied: (presetId, stage) =>
    set((state) => ({
      status: 'applied',
      appliedMeta: {
        ...state.appliedMeta,
        [presetId]: { stage, source: 'auto_learn' },
      },
      reviewHint: 'Overlay committed for this preset.',
    })),
  clearApplied: (presetId) =>
    set((state) => {
      const appliedMeta = { ...state.appliedMeta };
      delete appliedMeta[presetId];
      return { appliedMeta };
    }),
  bumpGeneration: () => {
    const next = get().requestGeneration + 1;
    set({ requestGeneration: next });
    return next;
  },
  resetForPreset: () =>
    set({
      status: 'idle',
      error: null,
      draft: null,
      logAttachNote: null,
      proposalTested: false,
      reviewHint: null,
    }),
}));
