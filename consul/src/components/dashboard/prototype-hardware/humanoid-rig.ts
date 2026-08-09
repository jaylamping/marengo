/**
 * PROTOTYPE — body-space definition for a stylized humanoid robot.
 *
 * +X right, +Y up, +Z toward viewer. Coordinates are metres.
 * Armor plates are box extents; joints are spheres that also host markers.
 */

export type Vec3 = readonly [number, number, number];

export type Limb = 'torso' | 'right_arm' | 'left_arm' | 'right_leg' | 'left_leg';

export type AnchorName =
  | 'head'
  | 'neck'
  | 'chest'
  | 'abdomen'
  | 'pelvis'
  | 'shoulder_r'
  | 'shoulder_l'
  | 'upper_arm_r'
  | 'upper_arm_l'
  | 'elbow_r'
  | 'elbow_l'
  | 'forearm_r'
  | 'forearm_l'
  | 'wrist_r'
  | 'wrist_l'
  | 'hip_r'
  | 'hip_l'
  | 'thigh_r'
  | 'thigh_l'
  | 'knee_r'
  | 'knee_l'
  | 'shin_r'
  | 'shin_l'
  | 'ankle_r'
  | 'ankle_l'
  | 'foot_r'
  | 'foot_l';

/** Articulation centres + plate midpoints. */
export const ANCHORS: Record<AnchorName, Vec3> = {
  head: [0, 1.62, 0.02],
  neck: [0, 1.48, 0],
  chest: [0, 1.3, 0.02],
  abdomen: [0, 1.1, 0.01],
  pelvis: [0, 0.94, 0],

  shoulder_r: [0.22, 1.4, 0],
  shoulder_l: [-0.22, 1.4, 0],
  upper_arm_r: [0.28, 1.22, 0.01],
  upper_arm_l: [-0.28, 1.22, 0.01],
  elbow_r: [0.3, 1.02, 0.02],
  elbow_l: [-0.3, 1.02, 0.02],
  forearm_r: [0.31, 0.86, 0.03],
  forearm_l: [-0.31, 0.86, 0.03],
  wrist_r: [0.32, 0.7, 0.03],
  wrist_l: [-0.32, 0.7, 0.03],

  hip_r: [0.11, 0.9, 0],
  hip_l: [-0.11, 0.9, 0],
  thigh_r: [0.12, 0.66, 0.01],
  thigh_l: [-0.12, 0.66, 0.01],
  knee_r: [0.12, 0.46, 0.02],
  knee_l: [-0.12, 0.46, 0.02],
  shin_r: [0.12, 0.26, 0.01],
  shin_l: [-0.12, 0.26, 0.01],
  ankle_r: [0.12, 0.08, 0.02],
  ankle_l: [-0.12, 0.08, 0.02],
  foot_r: [0.12, 0.04, 0.08],
  foot_l: [-0.12, 0.04, 0.08],
};

export type ArmorPlate = {
  anchor: AnchorName;
  size: Vec3;
  /** Euler XYZ radians. */
  rotation?: Vec3;
  offset?: Vec3;
  limb: Limb;
  kind: 'shell' | 'metal' | 'visor' | 'glow';
  /** Default box. Cylinders read as limb tubes; spheres as helmets. */
  shape?: 'box' | 'cylinder' | 'sphere';
};

/** Hard-shell plating — this is what makes it read as a robot, not a mannequin. */
export const ARMOR: ArmorPlate[] = [
  // Head / helmet
  {
    anchor: 'head',
    size: [0.17, 0.17, 0.17],
    limb: 'torso',
    kind: 'shell',
    shape: 'sphere',
  },
  {
    anchor: 'head',
    size: [0.13, 0.045, 0.04],
    offset: [0, 0.01, 0.075],
    limb: 'torso',
    kind: 'visor',
  },
  // Neck collar
  { anchor: 'neck', size: [0.1, 0.06, 0.1], limb: 'torso', kind: 'metal' },
  // Chest + abdomen + pelvis
  { anchor: 'chest', size: [0.36, 0.3, 0.2], limb: 'torso', kind: 'shell' },
  {
    anchor: 'chest',
    size: [0.16, 0.12, 0.04],
    offset: [0, 0.02, 0.105],
    limb: 'torso',
    kind: 'glow',
  },
  { anchor: 'abdomen', size: [0.26, 0.16, 0.16], limb: 'torso', kind: 'shell' },
  { anchor: 'pelvis', size: [0.3, 0.14, 0.18], limb: 'torso', kind: 'metal' },

  // Shoulders (pauldrons)
  {
    anchor: 'shoulder_r',
    size: [0.15, 0.1, 0.14],
    offset: [0.05, 0.04, 0],
    limb: 'right_arm',
    kind: 'metal',
  },
  {
    anchor: 'shoulder_l',
    size: [0.15, 0.1, 0.14],
    offset: [-0.05, 0.04, 0],
    limb: 'left_arm',
    kind: 'metal',
  },

  // Upper arms — cylinders
  {
    anchor: 'upper_arm_r',
    size: [0.09, 0.22, 0.09],
    limb: 'right_arm',
    kind: 'shell',
    shape: 'cylinder',
  },
  {
    anchor: 'upper_arm_l',
    size: [0.09, 0.22, 0.09],
    limb: 'left_arm',
    kind: 'shell',
    shape: 'cylinder',
  },
  // Forearms
  {
    anchor: 'forearm_r',
    size: [0.075, 0.2, 0.075],
    limb: 'right_arm',
    kind: 'shell',
    shape: 'cylinder',
  },
  {
    anchor: 'forearm_l',
    size: [0.075, 0.2, 0.075],
    limb: 'left_arm',
    kind: 'shell',
    shape: 'cylinder',
  },
  // Hands
  {
    anchor: 'wrist_r',
    size: [0.07, 0.08, 0.05],
    offset: [0, -0.02, 0.01],
    limb: 'right_arm',
    kind: 'metal',
  },
  {
    anchor: 'wrist_l',
    size: [0.07, 0.08, 0.05],
    offset: [0, -0.02, 0.01],
    limb: 'left_arm',
    kind: 'metal',
  },

  // Thighs / shins — cylinders
  {
    anchor: 'thigh_r',
    size: [0.11, 0.32, 0.11],
    limb: 'right_leg',
    kind: 'shell',
    shape: 'cylinder',
  },
  {
    anchor: 'thigh_l',
    size: [0.11, 0.32, 0.11],
    limb: 'left_leg',
    kind: 'shell',
    shape: 'cylinder',
  },
  {
    anchor: 'shin_r',
    size: [0.09, 0.28, 0.09],
    limb: 'right_leg',
    kind: 'shell',
    shape: 'cylinder',
  },
  {
    anchor: 'shin_l',
    size: [0.09, 0.28, 0.09],
    limb: 'left_leg',
    kind: 'shell',
    shape: 'cylinder',
  },
  // Feet
  {
    anchor: 'foot_r',
    size: [0.1, 0.06, 0.2],
    limb: 'right_leg',
    kind: 'metal',
  },
  {
    anchor: 'foot_l',
    size: [0.1, 0.06, 0.2],
    limb: 'left_leg',
    kind: 'metal',
  },
];

export type JointBall = {
  anchor: AnchorName;
  radius: number;
  limb: Limb;
};

/** Exposed spherical actuators between armor plates. */
export const JOINT_BALLS: JointBall[] = [
  { anchor: 'shoulder_r', radius: 0.055, limb: 'right_arm' },
  { anchor: 'elbow_r', radius: 0.045, limb: 'right_arm' },
  { anchor: 'wrist_r', radius: 0.035, limb: 'right_arm' },
  { anchor: 'shoulder_l', radius: 0.055, limb: 'left_arm' },
  { anchor: 'elbow_l', radius: 0.045, limb: 'left_arm' },
  { anchor: 'wrist_l', radius: 0.035, limb: 'left_arm' },
  { anchor: 'hip_r', radius: 0.055, limb: 'right_leg' },
  { anchor: 'knee_r', radius: 0.048, limb: 'right_leg' },
  { anchor: 'ankle_r', radius: 0.04, limb: 'right_leg' },
  { anchor: 'hip_l', radius: 0.055, limb: 'left_leg' },
  { anchor: 'knee_l', radius: 0.048, limb: 'left_leg' },
  { anchor: 'ankle_l', radius: 0.04, limb: 'left_leg' },
  { anchor: 'neck', radius: 0.04, limb: 'torso' },
];

/** Coloured marker sits slightly proud of its joint ball. */
export const MARKER_RADIUS = 0.05;

export const RIG_BOUNDS = (() => {
  let minY = Infinity;
  let maxY = -Infinity;
  let maxAbsX = 0;
  for (const point of Object.values(ANCHORS)) {
    minY = Math.min(minY, point[1]);
    maxY = Math.max(maxY, point[1]);
    maxAbsX = Math.max(maxAbsX, Math.abs(point[0]));
  }
  return {
    min: [-maxAbsX - 0.2, 0, -0.25] as Vec3,
    max: [maxAbsX + 0.2, maxY + 0.16, 0.25] as Vec3,
  };
})();

export const RIG_CENTER: Vec3 = [0, (RIG_BOUNDS.min[1] + RIG_BOUNDS.max[1]) / 2, 0];
export const RIG_HEIGHT = RIG_BOUNDS.max[1] - RIG_BOUNDS.min[1];
export const RIG_WIDTH = RIG_BOUNDS.max[0] - RIG_BOUNDS.min[0];
