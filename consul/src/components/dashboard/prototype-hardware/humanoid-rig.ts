/**
 * PROTOTYPE — skeleton the Hardware picker is drawn from.
 *
 * Body-space metres, Three.js convention: +X right, +Y up, +Z toward viewer.
 * Anchors double as joint marker positions so markers always land on a bone.
 */

export type Vec3 = readonly [number, number, number];

export const ANCHORS = {
  ankle_r: [0.1, 0.08, 0],
  ankle_l: [-0.1, 0.08, 0],
  toe_r: [0.1, 0.03, 0.11],
  toe_l: [-0.1, 0.03, 0.11],
  knee_r: [0.11, 0.5, 0.012],
  knee_l: [-0.11, 0.5, 0.012],
  hip_r: [0.1, 0.9, 0],
  hip_l: [-0.1, 0.9, 0],
  pelvis: [0, 0.92, 0],
  spine: [0, 1.12, 0],
  chest: [0, 1.32, 0],
  neck: [0, 1.44, 0],
  head: [0, 1.58, 0],
  shoulder_r: [0.19, 1.38, 0],
  shoulder_l: [-0.19, 1.38, 0],
  upper_arm_r: [0.25, 1.3, 0.02],
  upper_arm_l: [-0.25, 1.3, 0.02],
  elbow_r: [0.27, 1.06, 0.02],
  elbow_l: [-0.27, 1.06, 0.02],
  wrist_r: [0.28, 0.83, 0.03],
  wrist_l: [-0.28, 0.83, 0.03],
} satisfies Record<string, Vec3>;

export type AnchorName = keyof typeof ANCHORS;

export type Bone = {
  from: AnchorName;
  to: AnchorName;
  radius: number;
  /** Which limb this bone belongs to, for focus dimming. */
  limb: 'torso' | 'right_arm' | 'left_arm' | 'right_leg' | 'left_leg';
};

export const BONES: Bone[] = [
  { from: 'pelvis', to: 'spine', radius: 0.075, limb: 'torso' },
  { from: 'spine', to: 'chest', radius: 0.085, limb: 'torso' },
  { from: 'chest', to: 'neck', radius: 0.045, limb: 'torso' },
  { from: 'neck', to: 'head', radius: 0.062, limb: 'torso' },
  { from: 'chest', to: 'shoulder_r', radius: 0.05, limb: 'torso' },
  { from: 'chest', to: 'shoulder_l', radius: 0.05, limb: 'torso' },
  { from: 'pelvis', to: 'hip_r', radius: 0.055, limb: 'torso' },
  { from: 'pelvis', to: 'hip_l', radius: 0.055, limb: 'torso' },

  { from: 'shoulder_r', to: 'upper_arm_r', radius: 0.044, limb: 'right_arm' },
  { from: 'upper_arm_r', to: 'elbow_r', radius: 0.04, limb: 'right_arm' },
  { from: 'elbow_r', to: 'wrist_r', radius: 0.033, limb: 'right_arm' },

  { from: 'shoulder_l', to: 'upper_arm_l', radius: 0.044, limb: 'left_arm' },
  { from: 'upper_arm_l', to: 'elbow_l', radius: 0.04, limb: 'left_arm' },
  { from: 'elbow_l', to: 'wrist_l', radius: 0.033, limb: 'left_arm' },

  { from: 'hip_r', to: 'knee_r', radius: 0.055, limb: 'right_leg' },
  { from: 'knee_r', to: 'ankle_r', radius: 0.045, limb: 'right_leg' },
  { from: 'ankle_r', to: 'toe_r', radius: 0.038, limb: 'right_leg' },

  { from: 'hip_l', to: 'knee_l', radius: 0.055, limb: 'left_leg' },
  { from: 'knee_l', to: 'ankle_l', radius: 0.045, limb: 'left_leg' },
  { from: 'ankle_l', to: 'toe_l', radius: 0.038, limb: 'left_leg' },
];

/** Orbit/look-at target — roughly the sternum. */
export const RIG_FOCUS: Vec3 = [0, 1.05, 0];
