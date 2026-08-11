/**
 * Hardware commissioning facets + badge priority from wire truth.
 * Never consults localStorage / actuatorZeroStore — Ready = Verified on wire only.
 */

import { JointHomingState } from '@/gen/marengo/v1/marengo_pb';
import type { JointState, RobotState } from '@/gen/marengo/v1/marengo_pb';

/** Anatomical limbs from master `config/robot.yaml` (gateway does not yet expose limbs). */
export const MASTER_LIMBS: Readonly<Record<string, readonly string[]>> = {
  right_arm: [
    'right_shoulder_roll',
    'right_shoulder_pitch',
    'right_upper_arm_yaw',
    'right_elbow_pitch',
    'right_lower_arm_yaw',
  ],
  left_arm: [
    'left_shoulder_roll',
    'left_shoulder_pitch',
    'left_upper_arm_yaw',
    'left_elbow',
    'left_wrist',
  ],
} as const;

/** Single-badge class priority (highest wins): Fault > OutOfLimits > Offline > Active > Ready > Online. */
export type CommissioningBadge =
  | 'Fault'
  | 'OutOfLimits'
  | 'Offline'
  | 'Active'
  | 'Ready'
  | 'Online'
  | 'Unknown';

const BADGE_PRIORITY: readonly CommissioningBadge[] = [
  'Fault',
  'OutOfLimits',
  'Offline',
  'Active',
  'Ready',
  'Online',
  'Unknown',
] as const;

export type JointFacetSnapshot = {
  name: string;
  /** Live CAN / RobotState feedback present for this joint. */
  online: boolean;
  /** Present in motors.yaml map (built when online OR motor-mapped). */
  motorMapped: boolean;
  fault: number;
  outOfLimits: boolean;
  driveActive: boolean;
  /** Raw wire `homing_state` (camelCase or snake_case consumers). */
  homingState: unknown;
};

function normalizeHoming(raw: unknown): string | number | undefined {
  if (raw === undefined || raw === null) {
    return undefined;
  }
  if (typeof raw === 'number' && Number.isFinite(raw)) {
    return raw;
  }
  if (typeof raw === 'string') {
    return raw.trim().toUpperCase();
  }
  if (typeof raw === 'object' && raw !== null && 'value' in raw) {
    return normalizeHoming((raw as { value: unknown }).value);
  }
  return undefined;
}

/** True when publisher emits a non-UNSPECIFIED homing_state (Phase 3 wire live). */
export function wireFacetsLive(homingState: unknown): boolean {
  const normalized = normalizeHoming(homingState);
  if (normalized === undefined) {
    return false;
  }
  if (
    normalized === JointHomingState.UNSPECIFIED ||
    normalized === 'UNSPECIFIED' ||
    normalized === ''
  ) {
    return false;
  }
  return true;
}

/** Reference Ready = Verified on wire only. */
export function isReferenceReady(homingState: unknown): boolean {
  const normalized = normalizeHoming(homingState);
  return (
    normalized === JointHomingState.VERIFIED || normalized === 'VERIFIED'
  );
}

function isHomingFaulted(homingState: unknown): boolean {
  const normalized = normalizeHoming(homingState);
  return (
    normalized === JointHomingState.FAULTED || normalized === 'FAULTED'
  );
}

function isBuilt(joint: JointFacetSnapshot): boolean {
  return joint.online || joint.motorMapped;
}

function isReadyHealthy(joint: JointFacetSnapshot): boolean {
  return (
    isReferenceReady(joint.homingState) &&
    joint.fault === 0 &&
    !joint.outOfLimits &&
    !isHomingFaulted(joint.homingState)
  );
}

/**
 * Resolve the primary badge for a joint.
 * Priority: Fault > OutOfLimits > Offline > Active > Ready > Online.
 *
 * Unknown is only for description-only joints (no CAN map, no feedback).
 * Live feedback with UNSPECIFIED/missing homing_state is still Online (or
 * Active/Fault/…) — never Unknown. Motor-mapped but silent → Offline.
 */
export function resolveJointBadge(joint: JointFacetSnapshot): CommissioningBadge {
  const facetsLive = wireFacetsLive(joint.homingState);
  if (!joint.online && !facetsLive) {
    return joint.motorMapped ? 'Offline' : 'Unknown';
  }
  const faulted = joint.fault !== 0 || isHomingFaulted(joint.homingState);
  if (faulted) {
    return 'Fault';
  }
  if (joint.outOfLimits) {
    return 'OutOfLimits';
  }
  if (!joint.online) {
    return 'Offline';
  }
  if (joint.driveActive) {
    return 'Active';
  }
  if (facetsLive && isReferenceReady(joint.homingState)) {
    return 'Ready';
  }
  return 'Online';
}

export function badgePriorityIndex(badge: CommissioningBadge): number {
  const idx = BADGE_PRIORITY.indexOf(badge);
  return idx === -1 ? BADGE_PRIORITY.length : idx;
}

/** Highest-priority (worst) badge among members. Empty → Unknown. */
export function aggregateWorstBadge(
  badges: readonly CommissioningBadge[],
): CommissioningBadge {
  if (badges.length === 0) {
    return 'Unknown';
  }
  let worst: CommissioningBadge = badges[0]!;
  for (const badge of badges) {
    if (badgePriorityIndex(badge) < badgePriorityIndex(worst)) {
      worst = badge;
    }
  }
  return worst;
}

/** Limb Ready: every built member Ready+healthy; unbuilt Offline does not block. */
export function limbReady(members: readonly JointFacetSnapshot[]): boolean {
  const built = members.filter(isBuilt);
  if (built.length === 0) {
    return false;
  }
  return built.every(isReadyHealthy);
}

/** Robot Ready over master actuated joints (scope must not be used here). */
export function robotReady(masterJoints: readonly JointFacetSnapshot[]): boolean {
  return limbReady(masterJoints);
}

/**
 * Build a facet from a RobotState joint row.
 *
 * Online means live CAN feedback: Berthier omits joints without feedback from
 * `RobotState.joints`, so a missing wire row is Offline (not "present but silent").
 */
export function jointFacetFromWire(args: {
  name: string;
  motorMapped: boolean;
  wire: JointState | null | undefined;
}): JointFacetSnapshot {
  const wire = args.wire;
  if (!wire) {
    return {
      name: args.name,
      online: false,
      motorMapped: args.motorMapped,
      fault: 0,
      outOfLimits: false,
      driveActive: false,
      homingState: undefined,
    };
  }
  return {
    name: args.name,
    // Wire presence implies feedback-backed publish (see berthier publish_robot_state).
    online: true,
    motorMapped: args.motorMapped,
    fault: wire.fault ?? 0,
    outOfLimits: Boolean(wire.outOfLimits),
    driveActive: Boolean(wire.driveActive),
    homingState: wire.homingState,
  };
}

export function buildFacetSnapshots(args: {
  jointNames: readonly string[];
  motorMappedJoints: ReadonlySet<string>;
  robotState: RobotState | null | undefined;
}): JointFacetSnapshot[] {
  const byName = new Map(
    (args.robotState?.joints ?? []).map((j) => [j.name, j] as const),
  );
  return args.jointNames.map((name) =>
    jointFacetFromWire({
      name,
      motorMapped: args.motorMappedJoints.has(name),
      wire: byName.get(name),
    }),
  );
}

/** True when every joint with live feedback has non-UNSPECIFIED homing_state. */
export function robotWireFacetsLive(
  robotState: RobotState | null | undefined,
): boolean {
  const joints = robotState?.joints ?? [];
  if (joints.length === 0) {
    return false;
  }
  return joints.every((j) => wireFacetsLive(j.homingState));
}

export function commissioningBadgeLabel(badge: CommissioningBadge): string {
  return badge;
}

export function limbBadgeForMembers(
  members: readonly JointFacetSnapshot[],
): CommissioningBadge {
  const built = members.filter(isBuilt);
  if (built.length === 0) {
    return 'Offline';
  }
  return aggregateWorstBadge(built.map(resolveJointBadge));
}
