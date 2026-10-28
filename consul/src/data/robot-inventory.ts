export type InventoryGroup =
  | 'platform'
  | 'torso'
  | 'left_leg'
  | 'right_leg'
  | 'left_arm'
  | 'right_arm';

export type InventoryKind = 'actuator' | 'sensor' | 'device';

export type InventoryStatus =
  | 'Enabled'
  | 'Disabled'
  | 'Tuning'
  | 'Fault'
  | 'Offline'
  | 'Nominal';

export type InventoryItem = {
  id: number;
  name: string;
  group: InventoryGroup;
  kind: InventoryKind;
  status: InventoryStatus;
  value: string;
  limit: string;
  preset: string;
  node: string;
};

export const ACTUATOR_NODE_UNAVAILABLE = 'configuration unavailable';

export const INVENTORY_GROUP_ORDER: InventoryGroup[] = [
  'left_arm',
  'platform',
  'torso',
  'left_leg',
  'right_leg',
  'right_arm',
];

export const INVENTORY_GROUP_LABELS: Record<InventoryGroup, string> = {
  platform: 'Platform',
  torso: 'Torso',
  left_leg: 'Left Leg',
  right_leg: 'Right Leg',
  left_arm: 'Left Arm',
  right_arm: 'Right Arm',
};

function actuator(
  id: number,
  name: string,
  group: InventoryGroup,
  limit: string,
  preset: string,
): InventoryItem {
  return {
    id,
    name,
    group,
    kind: 'actuator',
    status: 'Offline',
    value: '—',
    limit,
    preset,
    node: ACTUATOR_NODE_UNAVAILABLE,
  };
}

function sensor(
  id: number,
  name: string,
  group: InventoryGroup,
  limit: string,
  preset: string,
  bus: string,
): InventoryItem {
  return {
    id,
    name,
    group,
    kind: 'sensor',
    status: 'Offline',
    value: '—',
    limit,
    preset,
    node: bus,
  };
}

function device(
  id: number,
  name: string,
  group: InventoryGroup,
  limit: string,
  preset: string,
  bus: string,
): InventoryItem {
  return {
    id,
    name,
    group,
    kind: 'device',
    status: 'Offline',
    value: '—',
    limit,
    preset,
    node: bus,
  };
}

/**
 * Catalog of planned limbs + platform sensors: names, groups and limits only.
 * Every row starts Offline with no value; status and value come only from the
 * live overlay (`useLiveInventory`), never from this file (UI-08).
 * CAN wiring comes from config snapshot.
 */
export const robotInventory: InventoryItem[] = [
  // Platform — compute, power, safety, perception (future)
  device(1, 'pi5_can_hat', 'platform', '—', 'bench_default', 'can0 · Waveshare 2-ch'),
  device(2, 'estop_chain', 'platform', '—', 'bench_default', 'gpio · NC chain'),
  sensor(3, 'battery_bms', 'platform', '42–54 V', 'bench_default', 'i2c · smart BMS'),
  sensor(4, 'torso_imu', 'platform', 'quat', 'bench_default', 'i2c-1 · 0x4b · BNO085'),
  sensor(5, 'head_rgb', 'platform', '—', 'unassigned', 'usb · future'),
  sensor(6, 'chest_depth', 'platform', '—', 'unassigned', 'usb · future'),

  // Torso
  actuator(7, 'waist_yaw', 'torso', '±2.71', 'unassigned'),

  // Left leg
  actuator(8, 'left_hip_yaw', 'left_leg', '±2.76', 'unassigned'),
  actuator(9, 'left_hip_roll', 'left_leg', '−0.52–2.97', 'unassigned'),
  actuator(10, 'left_hip_pitch', 'left_leg', '±2.69', 'unassigned'),
  actuator(11, 'left_knee', 'left_leg', '0–2.88', 'unassigned'),
  actuator(12, 'left_ankle_pitch', 'left_leg', '−1.0–0.8', 'last_session'),
  actuator(13, 'left_ankle_roll', 'left_leg', '±0.5', 'unassigned'),

  // Right leg
  actuator(14, 'right_hip_yaw', 'right_leg', '±2.76', 'unassigned'),
  actuator(15, 'right_hip_roll', 'right_leg', '−2.97–0.52', 'unassigned'),
  actuator(16, 'right_hip_pitch', 'right_leg', '±2.69', 'unassigned'),
  actuator(17, 'right_knee', 'right_leg', '0–2.88', 'unassigned'),
  actuator(18, 'right_ankle_pitch', 'right_leg', '−1.0–0.8', 'unassigned'),
  actuator(19, 'right_ankle_roll', 'right_leg', '±0.5', 'unassigned'),

  // Left arm — pitch before roll so catalog order matches CAN id SoT when unwired
  actuator(21, 'left_shoulder_pitch', 'left_arm', '−0.9–3.17', 'unassigned'),
  actuator(20, 'left_shoulder_roll', 'left_arm', '±1.57', 'unassigned'),
  actuator(22, 'left_upper_arm_yaw', 'left_arm', '±1.57', 'unassigned'),
  actuator(23, 'left_elbow', 'left_arm', '0–2.50', 'unassigned'),
  actuator(24, 'left_wrist', 'left_arm', '±1.6', 'unassigned'),

  // Right arm — pitch (CAN id 1) before roll (CAN id 2); lower-arm yaw CAN id 5
  actuator(26, 'right_shoulder_pitch', 'right_arm', '−0.9–3.17', 'bench_3dof'),
  actuator(25, 'right_shoulder_roll', 'right_arm', '±1.57', 'bench_3dof'),
  actuator(27, 'right_upper_arm_yaw', 'right_arm', '±1.57', 'unassigned'),
  actuator(28, 'right_elbow_pitch', 'right_arm', '0–1.20', 'unassigned'),
  actuator(29, 'right_lower_arm_yaw', 'right_arm', '±1.6', 'unassigned'),
];

/**
 * Limb group for a joint name: the catalog's group, or `platform` for a joint
 * the catalog does not list.
 */
export function inventoryGroupForJoint(jointName: string): InventoryGroup {
  return (
    robotInventory.find(
      (item) => item.kind === 'actuator' && item.name === jointName,
    )?.group ?? 'platform'
  );
}
