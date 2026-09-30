import type { ConfigSnapshotDto } from '@/lib/config-api';

export type AppliedJointLimits = {
  lower: number;
  upper: number;
  softLower: number;
  softUpper: number;
};

/**
 * Patch a config snapshot with Durable Set Limits hard/soft bounds so Consul
 * Disk fields match the just-ACKed live SoT without waiting on a refetch that
 * can race a throttled localStorage restore.
 */
export function applyConfigSnapshotLimits(
  snapshot: ConfigSnapshotDto | null | undefined,
  joint: string,
  limits: AppliedJointLimits,
): ConfigSnapshotDto | null {
  if (!snapshot) {
    return null;
  }
  const motors = snapshot.motors.map((motor) => {
    if (motor.joint !== joint) {
      return motor;
    }
    return {
      ...motor,
      bench: {
        ...motor.bench,
        position_lower_rad: limits.lower,
        position_upper_rad: limits.upper,
      },
    };
  });
  const hasSoft = snapshot.control_limits.some((row) => row.joint === joint);
  const control_limits = hasSoft
    ? snapshot.control_limits.map((row) => {
        if (row.joint !== joint) {
          return row;
        }
        return {
          ...row,
          position_soft_lower_rad: limits.softLower,
          position_soft_upper_rad: limits.softUpper,
        };
      })
    : [
        ...snapshot.control_limits,
        {
          joint,
          position_soft_lower_rad: limits.softLower,
          position_soft_upper_rad: limits.softUpper,
        },
      ];
  return {
    ...snapshot,
    motors,
    control_limits,
  };
}
