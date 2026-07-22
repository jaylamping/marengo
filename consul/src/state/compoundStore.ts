import { create as createZustand } from 'zustand';
import { compoundPresetById } from '@/data/compound-tests';

interface CompoundStore {
  selectedPresetId: string | null;
  trims: Record<string, number>;
  speedMultiplier: number;
  loop: boolean;
  progress: number; // 0 to 1
  isRunning: boolean;
  /** True while Stop Wave returnHome is in flight — blocks Teach Record. */
  returnHomePending: boolean;

  setSelectedPresetId: (id: string | null) => void;
  setTrim: (jointName: string, trim: number) => void;
  setSpeedMultiplier: (speed: number) => void;
  setLoop: (loop: boolean) => void;
  setProgress: (progress: number) => void;
  setIsRunning: (isRunning: boolean) => void;
  setReturnHomePending: (pending: boolean) => void;
  reset: () => void;
}

export const useCompoundStore = createZustand<CompoundStore>((set) => ({
  selectedPresetId: null,
  trims: {},
  speedMultiplier: 1.0,
  loop: false,
  progress: 0,
  isRunning: false,
  returnHomePending: false,

  setSelectedPresetId: (id) => {
    const preset = id ? compoundPresetById(id) : null;
    set({
      selectedPresetId: id,
      trims: {},
      progress: 0,
      isRunning: false,
      loop: preset ? preset.loop : false,
    });
  },
  setTrim: (jointName, trim) =>
    set((state) => ({ trims: { ...state.trims, [jointName]: trim } })),
  setSpeedMultiplier: (speed) => set({ speedMultiplier: speed }),
  setLoop: (loop) => set({ loop }),
  setProgress: (progress) => set({ progress }),
  setIsRunning: (isRunning) => set({ isRunning }),
  setReturnHomePending: (returnHomePending) => set({ returnHomePending }),
  reset: () =>
    set({
      selectedPresetId: null,
      trims: {},
      speedMultiplier: 1.0,
      loop: false,
      progress: 0,
      isRunning: false,
      returnHomePending: false,
    }),
}));
