import { create as createZustand } from 'zustand';
import { initialAckEpoch } from '@/lib/teach-calibration';
import {
  canApplyLandmarks,
  extractLandmarks,
  samplesHaveMotion,
  type TeachLandmark,
  type TeachSample,
} from '@/lib/teach-record';
import type { TeachSession } from '@/lib/teach-transit';

export const TEACH_STORAGE_KEY = 'marengo.teach.overlays.v1';

/** Persisted overlay: session is source of truth; preset is materialized at play. */
export interface TeachOverlayEntry {
  session: TeachSession;
  /** Epoch of last Apply or Acknowledge & keep. */
  ackedAtEpoch: number;
}

export interface TeachPersisted {
  liveCalibrationEpoch: number;
  overlays: Record<string, TeachOverlayEntry>;
}

export type FinishRecordingResult =
  | { kind: 'extracted'; landmarkCount: number }
  | { kind: 'no_motion' }
  | { kind: 'not_recording' };

/** Pure parse for tests + loadPersisted. Strips legacy frozen `preset` field. */
export function parseTeachPersisted(raw: string | null): TeachPersisted {
  if (!raw) {
    return { liveCalibrationEpoch: 0, overlays: {} };
  }
  try {
    const parsed = JSON.parse(raw) as Partial<TeachPersisted> & {
      overlays?: Record<
        string,
        TeachOverlayEntry & { ackedAtEpoch?: number; preset?: unknown }
      >;
    };
    const live =
      typeof parsed.liveCalibrationEpoch === 'number'
        ? parsed.liveCalibrationEpoch
        : 0;
    const overlays: Record<string, TeachOverlayEntry> = {};
    for (const [id, entry] of Object.entries(parsed.overlays ?? {})) {
      if (!entry?.session) continue;
      // Missing epoch fields: fail closed (force ack) — do not auto-ack to live.
      const failClosedEpoch = Math.max(0, live - 1);
      const session: TeachSession = {
        ...entry.session,
        calibrationEpoch:
          typeof entry.session.calibrationEpoch === 'number'
            ? entry.session.calibrationEpoch
            : failClosedEpoch,
      };
      overlays[id] = {
        session,
        ackedAtEpoch:
          typeof entry.ackedAtEpoch === 'number'
            ? entry.ackedAtEpoch
            : failClosedEpoch,
      };
    }
    return { liveCalibrationEpoch: live, overlays };
  } catch {
    return { liveCalibrationEpoch: 0, overlays: {} };
  }
}

function loadPersisted(): TeachPersisted {
  if (typeof window === 'undefined') {
    return { liveCalibrationEpoch: 0, overlays: {} };
  }
  return parseTeachPersisted(window.localStorage.getItem(TEACH_STORAGE_KEY));
}

function persistAll(
  liveCalibrationEpoch: number,
  overlays: Record<string, TeachOverlayEntry>
): boolean {
  if (typeof window === 'undefined') return true;
  try {
    const payload: TeachPersisted = { liveCalibrationEpoch, overlays };
    window.localStorage.setItem(TEACH_STORAGE_KEY, JSON.stringify(payload));
    return true;
  } catch {
    return false;
  }
}

interface TeachStore {
  recording: boolean;
  recordingPresetId: string | null;
  draftPresetId: string | null;
  gravityArmed: boolean;
  samples: TeachSample[];
  landmarks: TeachLandmark[];
  cadenceScale: number;
  settleDwellSec: number;
  lastError: string | null;
  liveCalibrationEpoch: number;
  overlays: Record<string, TeachOverlayEntry>;

  setGravityArmed: (armed: boolean) => void;
  startRecording: (presetId: string) => void;
  /**
   * Stop capture and extract landmarks for `joints`. Sole finalize path for
   * Stop / preset-switch / disconnect — never leave samples without landmarks.
   */
  finishRecording: (joints: string[]) => FinishRecordingResult;
  /** Abandon in-flight capture and draft (Back / tab / Reset). */
  cancelRecording: () => void;
  appendSample: (sample: TeachSample) => void;
  clearSamples: () => void;
  setLandmarks: (landmarks: TeachLandmark[]) => void;
  setLandmarkIncluded: (id: string, included: boolean) => void;
  setCadenceScale: (scale: number) => void;
  setSettleDwellSec: (sec: number) => void;
  setLastError: (msg: string | null) => void;
  /** Bump epoch after set-zero of teach joints — does not wipe overlays. */
  markCalibrationChanged: () => void;
  /** Keep overlay; allow Wave until the next set-zero mark. */
  acknowledgeCalibration: (presetId: string) => void;
  applyOverlay: (
    presetId: string,
    entry: Omit<TeachOverlayEntry, 'ackedAtEpoch'> & { ackedAtEpoch?: number }
  ) => boolean;
  clearOverlay: (presetId: string) => boolean;
  getOverlaySession: (presetId: string) => TeachSession | null;
  resetSession: () => void;
}

const initial = loadPersisted();

export const useTeachStore = createZustand<TeachStore>((set, get) => ({
  recording: false,
  recordingPresetId: null,
  draftPresetId: null,
  gravityArmed: false,
  samples: [],
  landmarks: [],
  cadenceScale: 1,
  settleDwellSec: 0.15,
  lastError: null,
  liveCalibrationEpoch: initial.liveCalibrationEpoch,
  overlays: initial.overlays,

  setGravityArmed: (gravityArmed) => set({ gravityArmed }),
  startRecording: (presetId) =>
    set({
      recording: true,
      recordingPresetId: presetId,
      draftPresetId: presetId,
      samples: [],
      landmarks: [],
      lastError: null,
    }),
  finishRecording: (joints) => {
    const state = get();
    if (!state.recordingPresetId) {
      return { kind: 'not_recording' };
    }
    const buf = state.samples;
    const draftPresetId = state.recordingPresetId;
    if (!samplesHaveMotion(buf, joints)) {
      set({
        recording: false,
        recordingPresetId: null,
        draftPresetId,
        landmarks: [],
        lastError: 'No motion in buffer. Nothing to apply.',
      });
      return { kind: 'no_motion' };
    }
    const extracted = extractLandmarks(buf, joints);
    set({
      recording: false,
      recordingPresetId: null,
      draftPresetId,
      landmarks: extracted,
      lastError: canApplyLandmarks(extracted)
        ? null
        : 'Landmark extraction failed. Do not apply this draft.',
    });
    return { kind: 'extracted', landmarkCount: extracted.length };
  },
  cancelRecording: () =>
    set({
      recording: false,
      recordingPresetId: null,
      draftPresetId: null,
      samples: [],
      landmarks: [],
      lastError: null,
    }),
  appendSample: (sample) =>
    set((state) => {
      if (!state.recordingPresetId) return state;
      const next =
        state.samples.length > 50_000
          ? [...state.samples.slice(-40_000), sample]
          : [...state.samples, sample];
      return { samples: next };
    }),
  clearSamples: () => set({ samples: [] }),
  setLandmarks: (landmarks) => set({ landmarks }),
  setLandmarkIncluded: (id, included) =>
    set((state) => ({
      landmarks: state.landmarks.map((l) =>
        l.id === id ? { ...l, included } : l
      ),
    })),
  setCadenceScale: (cadenceScale) => set({ cadenceScale }),
  setSettleDwellSec: (settleDwellSec) => set({ settleDwellSec }),
  setLastError: (lastError) => set({ lastError }),

  markCalibrationChanged: () => {
    const next = get().liveCalibrationEpoch + 1;
    const overlays = get().overlays;
    if (!persistAll(next, overlays)) {
      set({
        lastError:
          'Could not persist calibration mark — localStorage write blocked.',
      });
      return;
    }
    set({
      liveCalibrationEpoch: next,
      lastError:
        'Calibration marked dirty — re-record after set-zero (or Acknowledge & keep / Reset overlay).',
      samples: [],
      landmarks: [],
      recording: false,
      recordingPresetId: null,
      draftPresetId: null,
    });
  },

  acknowledgeCalibration: (presetId) => {
    const state = get();
    const entry = state.overlays[presetId];
    if (!entry) return;
    const overlays = {
      ...state.overlays,
      [presetId]: {
        ...entry,
        ackedAtEpoch: state.liveCalibrationEpoch,
      },
    };
    if (!persistAll(state.liveCalibrationEpoch, overlays)) {
      set({
        lastError: 'Acknowledge failed — localStorage write blocked.',
      });
      return;
    }
    set({ overlays, lastError: null });
  },

  applyOverlay: (presetId, entry) => {
    const live = get().liveCalibrationEpoch;
    const full: TeachOverlayEntry = {
      session: {
        ...entry.session,
        calibrationEpoch: entry.session.calibrationEpoch ?? live,
      },
      ackedAtEpoch: entry.ackedAtEpoch ?? initialAckEpoch(live),
    };
    const overlays = { ...get().overlays, [presetId]: full };
    if (!persistAll(live, overlays)) {
      set({
        lastError:
          'Apply failed — localStorage write blocked (quota or private mode).',
      });
      return false;
    }
    set({ overlays, lastError: null });
    return true;
  },

  clearOverlay: (presetId) => {
    const overlays = { ...get().overlays };
    delete overlays[presetId];
    if (!persistAll(get().liveCalibrationEpoch, overlays)) {
      set({
        lastError: 'Reset overlay failed — localStorage write blocked.',
      });
      return false;
    }
    set({ overlays });
    return true;
  },

  getOverlaySession: (presetId) => get().overlays[presetId]?.session ?? null,

  resetSession: () => get().cancelRecording(),
}));
