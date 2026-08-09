import type { CompletenessWarningDto } from '@/lib/hardware-api';
import type { ConfigSnapshotDto } from '@/lib/config-api';
import type { ActuatorLimitSnapshot, RobotState } from '@/gen/marengo/v1/marengo_pb';
import {
  MASTER_LIMBS,
  buildFacetSnapshots,
  resolveJointBadge,
  type CommissioningBadge,
  type JointFacetSnapshot,
} from '@/lib/commissioning';
import { liveJointEnvelope } from '@/state/actuatorStore';

export type HardwareJointRow = {
  joint: string;
  onCan: boolean;
  canId: number | null;
  canInterface: string | null;
  motorType: string | null;
  warningCount: number;
  warnings: CompletenessWarningDto[];
  liveRange: string;
  diskHardLower: number | null;
  diskHardUpper: number | null;
  diskSoftLower: number | null;
  diskSoftUpper: number | null;
  direction: number | null;
  badge: CommissioningBadge;
  facet: JointFacetSnapshot;
};

function formatRange(lower: number, upper: number): string {
  const fmt = (n: number) => (Number.isInteger(n) ? String(n) : n.toFixed(2));
  if (Math.abs(lower + upper) < 1e-6) {
    return `±${fmt(Math.abs(upper))}`;
  }
  return `${fmt(lower)}–${fmt(upper)}`;
}

function warningsForJoint(
  warnings: CompletenessWarningDto[],
  joint: string,
): CompletenessWarningDto[] {
  return warnings.filter((w) => w.joint === joint);
}

export function masterJointNames(snapshot: ConfigSnapshotDto | null): string[] {
  const fromSnapshot = snapshot?.joints ?? snapshot?.motors.map((m) => m.joint) ?? [];
  const limbMembers = Object.values(MASTER_LIMBS).flat();
  const seen = new Set<string>();
  const out: string[] = [];
  for (const name of [...fromSnapshot, ...limbMembers]) {
    if (!name || seen.has(name)) continue;
    seen.add(name);
    out.push(name);
  }
  return out;
}

export function buildHardwareFacetSnapshots(
  snapshot: ConfigSnapshotDto | null,
  robotState: RobotState | null | undefined,
): JointFacetSnapshot[] {
  const joints = masterJointNames(snapshot);
  const motorMapped = new Set(snapshot?.motors.map((m) => m.joint) ?? []);
  return buildFacetSnapshots({
    jointNames: joints,
    motorMappedJoints: motorMapped,
    robotState,
  });
}

export function buildHardwareRows(
  snapshot: ConfigSnapshotDto | null,
  completenessWarnings: CompletenessWarningDto[],
  limitSnapshot: ActuatorLimitSnapshot | null,
  robotState?: RobotState | null,
): HardwareJointRow[] {
  const joints = snapshot?.joints ?? snapshot?.motors.map((m) => m.joint) ?? [];
  const motorsByJoint = new Map(snapshot?.motors.map((m) => [m.joint, m] as const));
  const softByJoint = new Map(
    snapshot?.control_limits.map((c) => [c.joint, c] as const),
  );
  const facets = buildHardwareFacetSnapshots(snapshot, robotState);
  const facetByJoint = new Map(facets.map((f) => [f.name, f] as const));

  return joints.map((joint) => {
    const motor = motorsByJoint.get(joint);
    const soft = softByJoint.get(joint);
    const jointWarnings = warningsForJoint(completenessWarnings, joint);
    const envelope = liveJointEnvelope(joint, limitSnapshot);
    const liveRange = envelope
      ? formatRange(envelope.hardLowerRad, envelope.hardUpperRad)
      : motor
        ? formatRange(motor.bench.position_lower_rad, motor.bench.position_upper_rad)
        : '—';
    const facet = facetByJoint.get(joint) ?? {
      name: joint,
      online: false,
      motorMapped: motor != null,
      fault: 0,
      outOfLimits: false,
      driveActive: false,
      homingState: undefined,
    };

    return {
      joint,
      onCan: motor != null,
      canId: motor?.device_id ?? null,
      canInterface: motor?.can_interface ?? null,
      motorType: motor?.motor_type ?? null,
      warningCount: jointWarnings.length,
      warnings: jointWarnings,
      liveRange,
      diskHardLower: motor?.bench.position_lower_rad ?? null,
      diskHardUpper: motor?.bench.position_upper_rad ?? null,
      diskSoftLower: soft?.position_soft_lower_rad ?? null,
      diskSoftUpper: soft?.position_soft_upper_rad ?? null,
      direction: motor?.direction ?? null,
      badge: resolveJointBadge(facet),
      facet,
    };
  });
}

export function countCompletenessWarnings(warnings: CompletenessWarningDto[]): number {
  return warnings.length;
}
