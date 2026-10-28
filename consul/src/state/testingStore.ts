import { create as createZustand } from 'zustand';
import { create } from '@bufbuild/protobuf';
import {
  ControlMode,
  MitCommandBatchSchema,
  MitJointCommandSchema,
  type MitJointCommand,
} from '@/gen/marengo/v1/marengo_pb';
import {
  postTestingMitCommandBatch,
  postEnableCommand,
} from '@/lib/gateway-api';
import { useCompoundStore } from '@/state/compoundStore';
import { useRobotStore } from '@/state/robotStore';
import { useTeachStore } from '@/state/teachStore';

interface TestingStore {
  selectedJointNames: string[];
  mode: 'hold' | 'sweep';
  setpointRad: number;
  sweep: {
    startRad: number;
    endRad: number;
    stepRad: number;
    durationSec: number;
  };
  gains: Record<string, { kp: number; kd: number; ki: number; fc: number }>;
  dryRun: boolean;
  isRunning: boolean;

  selectJoint: (name: string) => void;
  deselectJoint: (name: string) => void;
  setMode: (mode: 'hold' | 'sweep') => void;
  setSetpoint: (rad: number) => void;
  setGain: (
    name: string,
    gain: { kp: number; kd: number; ki: number; fc: number },
  ) => void;
  toggleDryRun: () => void;
  setDryRun: (dryRun: boolean) => void;
  /** Rejects with the gateway error when the POST fails; isRunning is then false. */
  startTest: () => Promise<void>;
  /** Motion go-to-zero for selected joints — not HomingComplete / Mark READY. */
  returnHome: () => Promise<void>;
  /**
   * Stop a running hold where the joints are now: a POSITION batch at each
   * selected joint's measured position (UI-05). Without a measurement for
   * every joint it disables instead.
   */
  stopTest: () => Promise<void>;
  /**
   * E-stop / all-off path (not commissioning Enable). Runners are cancelled
   * before the request, so a failed POST never leaves a run going (UI-01).
   */
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

  selectJoint: (name) =>
    set((state) => ({
      selectedJointNames: [...state.selectedJointNames, name],
    })),
  deselectJoint: (name) =>
    set((state) => ({
      selectedJointNames: state.selectedJointNames.filter((n) => n !== name),
    })),
  setMode: (mode) => set({ mode }),
  setSetpoint: (setpointRad) => set({ setpointRad }),
  setGain: (name, gain) =>
    set((state) => ({ gains: { ...state.gains, [name]: gain } })),
  toggleDryRun: () => set((state) => ({ dryRun: !state.dryRun })),
  setDryRun: (dryRun) => set({ dryRun }),
  startTest: async () => {
    if (useTeachStore.getState().capture.kind === 'recording') {
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
      try {
        await postTestingMitCommandBatch(
          create(MitCommandBatchSchema, {
            timestampMs: BigInt(Date.now()),
            mode: ControlMode.POSITION,
            joints,
          }),
        );
      } catch (error) {
        // Nothing was accepted: the hold is not running (UI-13).
        set({ isRunning: false });
        throw error;
      }
    }
  },
  stopTest: async () => {
    const { selectedJointNames, gains, dryRun } = get();
    set({ isRunning: false });
    if (dryRun || selectedJointNames.length === 0) return;
    const robot = useRobotStore.getState().robotState;
    const joints: MitJointCommand[] = [];
    for (const name of selectedJointNames) {
      const measured = robot?.joints.find((joint) => joint.name === name);
      if (!measured) {
        await get().disable();
        return;
      }
      const gain = gains[name] || { kp: 0, kd: 0, ki: 0, fc: 0 };
      joints.push(
        create(MitJointCommandSchema, {
          name,
          kp: gain.kp,
          kd: gain.kd,
          ki: gain.ki,
          fc: gain.fc,
          position: measured.position,
          velocity: 0,
          torqueFf: 0,
        }),
      );
    }
    await postTestingMitCommandBatch(
      create(MitCommandBatchSchema, {
        timestampMs: BigInt(Date.now()),
        mode: ControlMode.POSITION,
        joints,
      }),
    );
  },

  returnHome: async () => {
    if (useTeachStore.getState().capture.kind === 'recording') {
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
      await postTestingMitCommandBatch(
        create(MitCommandBatchSchema, {
          timestampMs: BigInt(Date.now()),
          mode: ControlMode.POSITION,
          joints,
        }),
      );
    }
  },

  disable: async () => {
    set({ isRunning: false });
    useCompoundStore.getState().setIsRunning(false);
    await postEnableCommand(false);
  },

  dispatchGainUpdate: async (jointName: string) => {
    // Teach Record requires GravityComp — IMPEDANCE posts end it mid-capture.
    if (useTeachStore.getState().capture.kind === 'recording') {
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
      await postTestingMitCommandBatch(
        create(MitCommandBatchSchema, {
          timestampMs: BigInt(Date.now()),
          mode: ControlMode.IMPEDANCE,
          joints: [joint],
        }),
      );
    }
  },
}));
