import { create as createZustand } from 'zustand';

const STORAGE_KEY = 'marengo.consul.actuatorZeroed.v1';

function loadPersisted(): Record<string, true> {
  if (typeof window === 'undefined') return {};
  try {
    const raw = window.localStorage.getItem(STORAGE_KEY);
    if (!raw) return {};
    const parsed = JSON.parse(raw) as unknown;
    if (!parsed || typeof parsed !== 'object') return {};
    const out: Record<string, true> = {};
    for (const [joint, value] of Object.entries(parsed)) {
      if (value === true && joint.trim()) out[joint] = true;
    }
    return out;
  } catch {
    return {};
  }
}

function persist(zeroed: Record<string, true>): void {
  if (typeof window === 'undefined') return;
  try {
    window.localStorage.setItem(STORAGE_KEY, JSON.stringify(zeroed));
  } catch {
    // Quota / private mode — in-memory gate still works for the session.
  }
}

interface ActuatorZeroStore {
  /** Joints known zero'd (Set Zero success, or READY/ACTIVE sync). */
  zeroed: Record<string, true>;
  markZeroed: (joint: string) => void;
  markAllZeroed: (joints: string[]) => void;
  isZeroed: (joint: string) => boolean;
  /** Test helper — clears memory + storage. */
  reset: () => void;
}

export const useActuatorZeroStore = createZustand<ActuatorZeroStore>((set, get) => ({
  zeroed: loadPersisted(),
  markZeroed: (joint) => {
    const name = joint.trim();
    if (!name) return;
    set((state) => {
      if (state.zeroed[name]) return state;
      const zeroed = { ...state.zeroed, [name]: true as const };
      persist(zeroed);
      return { zeroed };
    });
  },
  markAllZeroed: (joints) => {
    const names = joints.map((j) => j.trim()).filter(Boolean);
    if (names.length === 0) return;
    set((state) => {
      let changed = false;
      const zeroed = { ...state.zeroed };
      for (const name of names) {
        if (!zeroed[name]) {
          zeroed[name] = true;
          changed = true;
        }
      }
      if (!changed) return state;
      persist(zeroed);
      return { zeroed };
    });
  },
  isZeroed: (joint) => Boolean(get().zeroed[joint.trim()]),
  reset: () => {
    persist({});
    set({ zeroed: {} });
  },
}));
