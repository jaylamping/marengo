/** PROTOTYPE mock — not live SoT */

import { ANCHORS, type AnchorName, type Vec3 } from './humanoid-rig';

export type FieldSource = 'urdf' | 'motors.yaml' | 'control.yaml' | 'homing.yaml';

export type MappedField = {
  id: string;
  label: string;
  value: string;
  source: FieldSource;
  warn?: boolean;
  incoming?: string;
};

export type Limb = 'torso' | 'right_arm' | 'left_arm' | 'right_leg' | 'left_leg';

export type ProtoJoint = {
  id: string;
  label: string;
  limb: Limb;
  onCan: boolean;
  completenessWarn: boolean;
  /** Skeleton anchor the joint marker sits on. */
  anchor: AnchorName;
  fields: MappedField[];
};

export const jointPosition = (joint: ProtoJoint): Vec3 => ANCHORS[joint.anchor];

const jointFields = (id: string): MappedField[] => [
  {
    id: `${id}-mass`,
    label: 'Link mass (kg)',
    value: id.includes('pitch') ? '1.12' : '0.84',
    source: 'urdf',
    warn: id.includes('elbow'),
  },
  {
    id: `${id}-com`,
    label: 'COM z (m)',
    value: '0.042',
    source: 'urdf',
  },
  {
    id: `${id}-hard-lo`,
    label: 'Hard limit min (rad)',
    value: '-1.57',
    source: 'urdf',
    incoming: id.includes('roll') ? '-2.0' : undefined,
  },
  {
    id: `${id}-hard-hi`,
    label: 'Hard limit max (rad)',
    value: '1.57',
    source: 'urdf',
  },
  {
    id: `${id}-can`,
    label: 'CAN id',
    value: id.includes('left') ? '—' : String(1 + (id.length % 4)),
    source: 'motors.yaml',
  },
  {
    id: `${id}-dir`,
    label: 'Direction',
    value: '1',
    source: 'motors.yaml',
  },
  {
    id: `${id}-kp`,
    label: 'kp (durable)',
    value: '18',
    source: 'control.yaml',
  },
  {
    id: `${id}-kd`,
    label: 'kd (durable)',
    value: '3',
    source: 'control.yaml',
  },
  {
    id: `${id}-home`,
    label: 'Homing offset (rad)',
    value: '0.0',
    source: 'homing.yaml',
  },
];

export const PROTO_JOINTS: ProtoJoint[] = [
  {
    id: 'right_shoulder_roll',
    label: 'Right shoulder roll',
    limb: 'right_arm',
    onCan: true,
    completenessWarn: false,
    anchor: 'shoulder_r',
    fields: jointFields('right_shoulder_roll'),
  },
  {
    id: 'right_shoulder_pitch',
    label: 'Right shoulder pitch',
    limb: 'right_arm',
    onCan: true,
    completenessWarn: false,
    anchor: 'upper_arm_r',
    fields: jointFields('right_shoulder_pitch'),
  },
  {
    id: 'right_elbow_pitch',
    label: 'Right elbow pitch',
    limb: 'right_arm',
    onCan: true,
    completenessWarn: true,
    anchor: 'elbow_r',
    fields: jointFields('right_elbow_pitch'),
  },
  {
    id: 'left_shoulder_roll',
    label: 'Left shoulder roll',
    limb: 'left_arm',
    onCan: false,
    completenessWarn: true,
    anchor: 'shoulder_l',
    fields: jointFields('left_shoulder_roll'),
  },
  {
    id: 'left_shoulder_pitch',
    label: 'Left shoulder pitch',
    limb: 'left_arm',
    onCan: false,
    completenessWarn: true,
    anchor: 'upper_arm_l',
    fields: jointFields('left_shoulder_pitch'),
  },
  {
    id: 'right_hip_pitch',
    label: 'Right hip pitch',
    limb: 'right_leg',
    onCan: false,
    completenessWarn: true,
    anchor: 'hip_r',
    fields: jointFields('right_hip_pitch'),
  },
  {
    id: 'left_hip_pitch',
    label: 'Left hip pitch',
    limb: 'left_leg',
    onCan: false,
    completenessWarn: true,
    anchor: 'hip_l',
    fields: jointFields('left_hip_pitch'),
  },
];

export const WARN_COUNT = PROTO_JOINTS.filter((j) => j.completenessWarn).length;
