import type { ActuatorLimitSnapshot } from '@/gen/marengo/v1/marengo_pb';
import type { ConfigSnapshotDto } from '@/lib/config-api';
import type { InventoryItem } from '@/data/robot-inventory';
import { deriveMembershipPreset } from '@/lib/bringup-presets';
import { liveJointEnvelope } from '@/state/actuatorStore';

function formatBenchLimit(lower: number, upper: number): string {
  const fmt = (n: number) => (Number.isInteger(n) ? String(n) : n.toFixed(2));
  if (Math.abs(lower + upper) < 1e-6) {
    return `±${fmt(Math.abs(upper))}`;
  }
  return `${fmt(lower)}–${fmt(upper)}`;
}

/**
 * Overlay gateway config + live Davout limits onto static inventory rows.
 *
 * Range SoT (ADR 0012): prefer ActuatorLimitSnapshot hard (URDF ∩ bench).
 * Disk config soft/bench is fallback only when the live snapshot is missing.
 */
export function enrichInventory(
  base: InventoryItem[],
  snapshot: ConfigSnapshotDto | null,
  limitSnapshot: ActuatorLimitSnapshot | null = null,
): InventoryItem[] {
  if (!snapshot && !limitSnapshot) {
    return base;
  }

  const motorsByJoint = new Map(
    (snapshot?.motors ?? []).map((m) => [m.joint, m] as const),
  );
  const softByJoint = new Map(
    (snapshot?.control_limits ?? []).map((c) => [c.joint, c] as const),
  );

  return base.map((row) => {
    if (row.kind !== 'actuator') {
      return row;
    }
    const motor = motorsByJoint.get(row.name);
    const envelope = liveJointEnvelope(row.name, limitSnapshot);

    let limit = row.limit;
    if (envelope) {
      limit = formatBenchLimit(envelope.hardLowerRad, envelope.hardUpperRad);
    } else if (motor) {
      const soft = softByJoint.get(row.name);
      const lower = soft?.position_soft_lower_rad ?? motor.bench.position_lower_rad;
      const upper = soft?.position_soft_upper_rad ?? motor.bench.position_upper_rad;
      limit = formatBenchLimit(lower, upper);
    }

    if (!motor && !envelope) {
      return row;
    }

    const membershipPreset =
      snapshot != null
        ? deriveMembershipPreset(row.name, snapshot.profile, snapshot.joints)
        : null;

    return {
      ...row,
      limit,
      node: motor
        ? `${motor.motor_type.toUpperCase()} · ${motor.can_interface} · id ${motor.device_id}`
        : row.node,
      preset: membershipPreset ?? row.preset,
    };
  });
}
