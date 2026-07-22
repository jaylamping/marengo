import type { ConfigSnapshotDto } from '@/lib/config-api';
import type { InventoryItem } from '@/data/robot-inventory';
import { deriveMembershipPreset } from '@/lib/bringup-presets';

function formatBenchLimit(lower: number, upper: number): string {
  const fmt = (n: number) => (Number.isInteger(n) ? String(n) : n.toFixed(2));
  if (Math.abs(lower + upper) < 1e-6) {
    return `±${fmt(Math.abs(upper))}`;
  }
  return `${fmt(lower)}–${fmt(upper)}`;
}

/**
 * Overlay gateway config onto static inventory rows (limits + CAN node + profile preset).
 * Returns a new array; does not mutate `base`.
 */
export function enrichInventory(
  base: InventoryItem[],
  snapshot: ConfigSnapshotDto | null,
): InventoryItem[] {
  if (!snapshot) {
    return base;
  }

  const motorsByJoint = new Map(snapshot.motors.map((m) => [m.joint, m] as const));
  const softByJoint = new Map(
    snapshot.control_limits.map((c) => [c.joint, c] as const),
  );

  return base.map((row) => {
    if (row.kind !== 'actuator') {
      return row;
    }
    const motor = motorsByJoint.get(row.name);
    if (!motor) {
      return row;
    }

    const soft = softByJoint.get(row.name);
    const lower = soft?.position_soft_lower_rad ?? motor.bench.position_lower_rad;
    const upper = soft?.position_soft_upper_rad ?? motor.bench.position_upper_rad;

    const membershipPreset = deriveMembershipPreset(
      row.name,
      snapshot.profile,
      snapshot.joints,
    );

    return {
      ...row,
      limit: formatBenchLimit(lower, upper),
      node: `${motor.motor_type.toUpperCase()} · ${motor.can_interface} · id ${motor.device_id}`,
      // Membership-derived bench_* is SoT for mapped presets; catalog tags stay on the row.
      preset: membershipPreset ?? row.preset,
    };
  });
}
