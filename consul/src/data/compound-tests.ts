export interface Keyframe {
  targetRad: number;
  durationSec: number;
}

export interface CompoundTestPreset {
  id: string;
  name: string;
  description: string;
  joints: string[];
  keyframes: Record<string, Keyframe[]>;
  loop: boolean;
}

export const COMPOUND_TEST_PRESETS: CompoundTestPreset[] = [
  {
    id: 'wave',
    name: 'Wave',
    description: 'Arm out, waving roll back and forth.',
    joints: ['right_shoulder_pitch', 'right_shoulder_roll'],
    loop: true,
    keyframes: {
      right_shoulder_pitch: [
        { targetRad: 1.2, durationSec: 2.0 },
        { targetRad: 1.2, durationSec: 2.0 },
        { targetRad: 1.2, durationSec: 2.0 },
        { targetRad: 1.2, durationSec: 2.0 },
      ],
      right_shoulder_roll: [
        { targetRad: 0.6, durationSec: 2.0 },
        { targetRad: 2.6, durationSec: 2.0 },
        { targetRad: 0.6, durationSec: 2.0 },
        { targetRad: 2.6, durationSec: 2.0 },
      ],
    },
  },
  {
    id: 'arm_out_forward',
    name: 'Arm Out Forward',
    description: 'Raises the arm straight out forward.',
    joints: ['right_shoulder_pitch', 'right_shoulder_roll'],
    loop: false,
    keyframes: {
      right_shoulder_pitch: [
        { targetRad: 1.4, durationSec: 1.5 },
      ],
      right_shoulder_roll: [
        { targetRad: 1.57, durationSec: 1.5 },
      ],
    },
  },
  {
    id: 'arm_fully_up',
    name: 'Arm Fully Up',
    description: 'Raises the arm fully up.',
    joints: ['right_shoulder_pitch', 'right_shoulder_roll'],
    loop: false,
    keyframes: {
      right_shoulder_pitch: [
        { targetRad: 2.6, durationSec: 2.0 },
      ],
      right_shoulder_roll: [
        { targetRad: 1.57, durationSec: 2.0 },
      ],
    },
  },
];
