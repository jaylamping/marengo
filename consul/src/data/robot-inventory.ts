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
  status: InventoryStatus,
  value: string,
  limit: string,
  preset: string,
  motor: string,
  canId: number,
): InventoryItem {
  return {
    id,
    name,
    group,
    kind: 'actuator',
    status,
    value,
    limit,
    preset,
    node: `${motor.toUpperCase()} · can0 · id ${canId}`,
  };
}

function sensor(
  id: number,
  name: string,
  group: InventoryGroup,
  status: InventoryStatus,
  value: string,
  limit: string,
  preset: string,
  bus: string,
): InventoryItem {
  return {
    id,
    name,
    group,
    kind: 'sensor',
    status,
    value,
    limit,
    preset,
    node: bus,
  };
}

function device(
  id: number,
  name: string,
  group: InventoryGroup,
  status: InventoryStatus,
  value: string,
  limit: string,
  preset: string,
  bus: string,
): InventoryItem {
  return {
    id,
    name,
    group,
    kind: 'device',
    status,
    value,
    limit,
    preset,
    node: bus,
  };
}

/** Dummy inventory aligned with motors_humanoid.yaml + planned platform sensors. */
export const robotInventory: InventoryItem[] = [
  // Platform — compute, power, safety, perception (future)
  device(1, 'pi5_can_hat', 'platform', 'Enabled', 'up', '—', 'bench_default', 'can0 · Waveshare 2-ch'),
  device(2, 'estop_chain', 'platform', 'Nominal', 'closed', '—', 'bench_default', 'gpio · NC chain'),
  sensor(3, 'battery_bms', 'platform', 'Nominal', '48.1 V · 87%', '42–54 V', 'bench_default', 'i2c · smart BMS'),
  sensor(4, 'torso_imu', 'platform', 'Offline', '—', 'quat', 'bench_default', 'i2c-1 · 0x4b · BNO085'),
  sensor(5, 'head_rgb', 'platform', 'Offline', '—', '—', 'unassigned', 'usb · future'),
  sensor(6, 'chest_depth', 'platform', 'Offline', '—', '—', 'unassigned', 'usb · future'),

  // Torso
  actuator(7, 'waist_yaw', 'torso', 'Offline', '0.00', '±2.71', 'unassigned', 'rs03', 13),

  // Left leg
  actuator(8, 'left_hip_yaw', 'left_leg', 'Offline', '—', '±2.76', 'unassigned', 'rs03', 1),
  actuator(9, 'left_hip_roll', 'left_leg', 'Offline', '—', '−0.52–2.97', 'unassigned', 'rs03', 2),
  actuator(10, 'left_hip_pitch', 'left_leg', 'Offline', '—', '±2.69', 'unassigned', 'rs04', 3),
  actuator(11, 'left_knee', 'left_leg', 'Offline', '—', '0–2.88', 'unassigned', 'rs04', 4),
  actuator(12, 'left_ankle_pitch', 'left_leg', 'Fault', '0.18', '−1.0–0.8', 'last_session', 'rs02', 5),
  actuator(13, 'left_ankle_roll', 'left_leg', 'Offline', '—', '±0.5', 'unassigned', 'rs02', 6),

  // Right leg
  actuator(14, 'right_hip_yaw', 'right_leg', 'Offline', '—', '±2.76', 'unassigned', 'rs03', 7),
  actuator(15, 'right_hip_roll', 'right_leg', 'Offline', '—', '−2.97–0.52', 'unassigned', 'rs03', 8),
  actuator(16, 'right_hip_pitch', 'right_leg', 'Offline', '—', '±2.69', 'unassigned', 'rs04', 9),
  actuator(17, 'right_knee', 'right_leg', 'Offline', '—', '0–2.88', 'unassigned', 'rs04', 10),
  actuator(18, 'right_ankle_pitch', 'right_leg', 'Offline', '—', '−1.0–0.8', 'unassigned', 'rs02', 11),
  actuator(19, 'right_ankle_roll', 'right_leg', 'Offline', '—', '±0.5', 'unassigned', 'rs02', 12),

  // Left arm — 4 DOF bench slice wired on can0 ids 1–4 (humanoid ids 14–17)
  actuator(20, 'left_shoulder_roll', 'left_arm', 'Offline', '—', '±1.57', 'unassigned', 'rs03', 14),
  actuator(21, 'left_shoulder_pitch', 'left_arm', 'Offline', '—', '−0.9–3.17', 'unassigned', 'rs03', 15),
  actuator(22, 'left_upper_arm_yaw', 'left_arm', 'Offline', '—', '±1.57', 'unassigned', 'rs02', 16),
  actuator(23, 'left_elbow', 'left_arm', 'Offline', '—', '0–2.50', 'unassigned', 'rs02', 17),
  actuator(24, 'left_wrist', 'left_arm', 'Offline', '—', '±1.6', 'unassigned', 'rs00', 18),

  // Right arm
  actuator(25, 'right_shoulder_roll', 'right_arm', 'Enabled', '—', '±1.57', 'bench_3dof', 'rs03', 1),
  actuator(26, 'right_shoulder_pitch', 'right_arm', 'Enabled', '—', '−0.9–3.17', 'bench_3dof', 'rs03', 2),
  actuator(27, 'right_upper_arm_yaw', 'right_arm', 'Enabled', '—', '±1.57', 'bench_4dof', 'rs02', 3),
  actuator(28, 'right_elbow_pitch', 'right_arm', 'Enabled', '—', '0–1.20', 'bench_4dof', 'rs02', 4),
  actuator(29, 'right_lower_arm_yaw', 'right_arm', 'Offline', '—', '±1.6', 'unassigned', 'rs00', 23),
];

export function countByStatus(status: InventoryStatus): number {
  return robotInventory.filter((item) => item.status === status).length;
}

export function countUnconfigured(): number {
  return robotInventory.filter((item) => item.preset === 'unassigned').length;
}

export function countActuatorsEnabled(): number {
  return robotInventory.filter(
    (item) => item.kind === 'actuator' && item.status === 'Enabled',
  ).length;
}
