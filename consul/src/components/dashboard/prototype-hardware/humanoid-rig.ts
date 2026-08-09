/**
 * PROTOTYPE — Bender-style humanoid layout (throwaway homage, not production).
 *
 * Body-space metres: +X right, +Y up, +Z toward viewer.
 * Anchors host status LEDs for the Hardware joint picker.
 */

export type Vec3 = readonly [number, number, number];

export type Limb = 'torso' | 'right_arm' | 'left_arm' | 'right_leg' | 'left_leg';

export type AnchorName =
  | 'head'
  | 'shoulder_r'
  | 'shoulder_l'
  | 'upper_arm_r'
  | 'upper_arm_l'
  | 'elbow_r'
  | 'elbow_l'
  | 'wrist_r'
  | 'wrist_l'
  | 'hip_r'
  | 'hip_l'
  | 'knee_r'
  | 'knee_l'
  | 'ankle_r'
  | 'ankle_l';

/** Joint / marker centres on the Bender silhouette. */
export const ANCHORS: Record<AnchorName, Vec3> = {
  head: [0, 1.52, 0.02],
  shoulder_r: [0.28, 1.22, 0],
  shoulder_l: [-0.28, 1.22, 0],
  upper_arm_r: [0.31, 1.07, 0.01],
  upper_arm_l: [-0.31, 1.07, 0.01],
  elbow_r: [0.34, 0.92, 0.02],
  elbow_l: [-0.34, 0.92, 0.02],
  wrist_r: [0.36, 0.62, 0.02],
  wrist_l: [-0.36, 0.62, 0.02],
  hip_r: [0.1, 0.72, 0],
  hip_l: [-0.1, 0.72, 0],
  knee_r: [0.12, 0.42, 0.02],
  knee_l: [-0.12, 0.42, 0.02],
  ankle_r: [0.12, 0.14, 0.02],
  ankle_l: [-0.12, 0.14, 0.02],
};

/** Coloured picker LED radius. */
export const MARKER_RADIUS = 0.048;

export const RIG_BOUNDS = {
  min: [-0.55, 0, -0.35] as Vec3,
  max: [0.55, 1.85, 0.35] as Vec3,
};

export const RIG_CENTER: Vec3 = [0, 0.9, 0];
export const RIG_HEIGHT = RIG_BOUNDS.max[1] - RIG_BOUNDS.min[1];
export const RIG_WIDTH = RIG_BOUNDS.max[0] - RIG_BOUNDS.min[0];

/** Bender palette — flat metallic cartoon. */
export const BENDER = {
  metal: 0xa8b8c4,
  metalDark: 0x7a8a96,
  metalLight: 0xc5d0d8,
  cream: 0xffffc2,
  pupil: 0x101010,
  outline: 0x0a0a0a,
  doorKnob: 0x5a6570,
} as const;
