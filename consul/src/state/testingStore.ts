import { create as createZustand } from 'zustand';
import { create } from '@bufbuild/protobuf';
import {
  ControlMode,
  MitCommandBatchSchema,
  MitJointCommandSchema,
  type MitJointCommand,
} from '@/gen/marengo/v1/marengo_pb';
import { postTestingMitCommandBatch, postEnableCommand } from '@/lib/gateway-api';
import { useCompoundStore } from '@/state/compoundStore';
import { useTeachStore } from '@/state/teachStore';

interface TestingStore {
  selectedJointNames: string[];
  mode: 'hold' | 'sweep';
  setpointRad: number;
  sweep: { startRad: number; endRad: number; stepRad: number; durationSec: number };
  gains: Record<string, { kp: number; kd: number; ki: number; fc: number }>;
  dryRun: boolean;
  isRunning: boolean;

  selectJoint: (name: string) => void;
  deselectJoint: (name: string) => void;
  setMode: (mode: 'hold' | 'sweep') => void;
  setSetpoint: (rad: number) => void;
  setGain: (name: string, gain: { kp: number; kd: number; ki: number; fc: number }) => void;
  toggleDryRun: () => void;
  startTest: () => Promise<void>;
  returnHome: () => Promise<void>;
  stopTest: () => void;
  enable: () => Promise<void>;
  disable: () => Promise<void>;
  dispatchGainUpdate: (jointName: string) => Promise<void>;
}

export const useTestingStore = createZustand<TestingStore>((set, get) => ({
  selectedJointNames: [],
  mode: 'hold',
  setpointRad: 0,
  sweep: { startRad: 0, endRad: 0, stepRad: 0, durationSec: 0 },
  gains: {},
  dryRun: true,
  isRunning: false,

  selectJoint: (name) => set((state) => ({ selectedJointNames: [...state.selectedJointNames, name] })),
  deselectJoint: (name) => set((state) => ({ selectedJointNames: state.selectedJointNames.filter((n) => n !== name) })),
  setMode: (mode) => set({ mode }),
  setSetpoint: (setpointRad) => set({ setpointRad }),
  setGain: (name, gain) => set((state) => ({ gains: { ...state.gains, [name]: gain } })),
  toggleDryRun: () => set((state) => ({ dryRun: !state.dryRun })),
  startTest: async () => {
    if (useTeachStore.getState().recording) {
      return;
    }
    const { selectedJointNames, gains, setpointRad, dryRun } = get();
    set({ isRunning: true });

    const joints: MitJointCommand[] = selectedJointNames.map((name) => {
      const gain = gains[name] || { kp: 0, kd: 0, ki: 0, fc: 0 };
      return create(MitJointCommandSchema, {
        name,
        kp: gain.kp,
        kd: gain.kd,
        ki: gain.ki,
        fc: gain.fc,
        position: setpointRad,
        velocity: 0,
        torqueFf: 0,
      });
    });


    if (!dryRun) {
      // POSITION ends GravityComp — clear teach preflight checkbox.
      useTeachStore.getState().setGravityArmed(false);
      await postTestingMitCommandBatch(create(MitCommandBatchSchema, {
        timestampMs: BigInt(Date.now()),
        mode: ControlMode.POSITION,
        joints,
      }));
    }
  },
  stopTest: () => set({ isRunning: false }),

  returnHome: async () => {
    if (useTeachStore.getState().recording) {
      return;
    }
    const { selectedJointNames, gains, dryRun } = get();
    set({ isRunning: false, setpointRad: 0 });

    const joints: MitJointCommand[] = selectedJointNames.map((name) => {
      const gain = gains[name] || { kp: 0, kd: 0, ki: 0, fc: 0 };
      return create(MitJointCommandSchema, {
        name,
        kp: gain.kp,
        kd: gain.kd,
        ki: gain.ki,
        fc: gain.fc,
        position: 0,
        velocity: 0,
        torqueFf: 0,
      });
    });

    if (!dryRun) {
      useTeachStore.getState().setGravityArmed(false);
      await postTestingMitCommandBatch(create(MitCommandBatchSchema, {
        timestampMs: BigInt(Date.now()),
        mode: ControlMode.POSITION,
        joints,
      }));
    }
  },

  enable: async () => {
    await postEnableCommand(true);
  },

  disable: async () => {
    await postEnableCommand(false);
    set({ isRunning: false });
    useCompoundStore.getState().setIsRunning(false);
  },

  dispatchGainUpdate: async (jointName: string) => {
    // Teach Record requires GravityComp — IMPEDANCE posts end it mid-capture.
    if (useTeachStore.getState().recording) {
      return;
    }
    const { gains, setpointRad, dryRun } = get();
    const gain = gains[jointName];
    if (!gain) {
      return;
    }
    const joint = create(MitJointCommandSchema, {
      name: jointName,
      kp: gain.kp,
      kd: gain.kd,
      ki: gain.ki,
      fc: gain.fc,
      position: setpointRad,
      velocity: 0,
      torqueFf: 0,
    });
    if (!dryRun) {
      useTeachStore.getState().setGravityArmed(false);
      await postTestingMitCommandBatch(create(MitCommandBatchSchema, {
        timestampMs: BigInt(Date.now()),
        mode: ControlMode.IMPEDANCE,
        joints: [joint],
      }));
    }
  },
}));
