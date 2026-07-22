import type { Fault, RobotState, SafetyState } from '@/gen/marengo/v1/marengo_pb';
import type { ConfigSnapshotDto } from '@/lib/config-api';
import type { OperationalModeLabel } from '@/state/robotStore';

export function formatSafetyFaults(faults: Fault[]): string {
  if (faults.length === 0) return '';
  return faults
    .map((f) => {
      const joint = f.joint?.trim() ? `${f.joint}: ` : '';
      const code = f.code?.trim() ? `[${f.code}] ` : '';
      return `${joint}${code}${f.message || 'fault'}`.trim();
    })
    .join(' · ');
}

const POS_SLACK_RAD = 0.02;

export type JointPoseHint = {
  name: string;
  position: number;
  lower: number;
  upper: number;
};

/** Compare live poses to motors.yaml bench (Consul snapshot). Davout hard = URDF ∩ bench. */
export function diagnoseEnableDisabledTrip(
  robotState: RobotState | null | undefined,
  config: ConfigSnapshotDto | null | undefined,
): string {
  const joints = robotState?.joints ?? [];
  if (joints.length === 0) {
    return 'Enable posted, but motors returned to DISABLED (hard-limit / safety trip — check Pi journal).';
  }

  const motorsByJoint = new Map(
    (config?.motors ?? []).map((m) => [m.joint, m.bench] as const),
  );

  const outsideMotors: JointPoseHint[] = [];
  const withinMotors: JointPoseHint[] = [];

  for (const j of joints) {
    const bench = motorsByJoint.get(j.name);
    if (!bench) continue;
    const hint: JointPoseHint = {
      name: j.name,
      position: j.position,
      lower: bench.position_lower_rad,
      upper: bench.position_upper_rad,
    };
    if (
      j.position < bench.position_lower_rad - POS_SLACK_RAD ||
      j.position > bench.position_upper_rad + POS_SLACK_RAD
    ) {
      outsideMotors.push(hint);
    } else {
      withinMotors.push(hint);
    }
  }

  if (outsideMotors.length > 0) {
    const detail = outsideMotors
      .map(
        (h) =>
          `${h.name} at ${h.position.toFixed(3)} outside motors bench [${h.lower.toFixed(2)}, ${h.upper.toFixed(2)}]`,
      )
      .join(' · ');
    return `Enable posted, but motors returned to DISABLED — ${detail}.`;
  }

  if (withinMotors.length > 0) {
    const detail = withinMotors
      .map((h) => `${h.name}=${h.position.toFixed(3)}`)
      .join(', ');
    return (
      `Enable posted, but motors returned to DISABLED — poses within motors.yaml (${detail}), ` +
      `so Davout likely hit URDF hard clamp (effective = URDF ∩ bench). Sync URDF + restart marengo-pi.`
    );
  }

  return 'Enable posted, but motors returned to DISABLED (hard-limit / safety trip — check Pi journal).';
}

/**
 * Interpret post-Enable telemetry for operator-facing status.
 * Returns null while still waiting for a conclusive outcome.
 */
export function interpretPostEnableWatch(args: {
  elapsedMs: number;
  operationalMode: OperationalModeLabel;
  safetyState: SafetyState | null;
  robotState?: RobotState | null;
  config?: ConfigSnapshotDto | null;
}): { done: boolean; message: string | null; kind: 'ok' | 'error' | 'pending' } {
  const faults = args.safetyState?.activeFaults ?? [];
  if (faults.length > 0) {
    return {
      done: true,
      kind: 'error',
      message: `Enable tripped safety — ${formatSafetyFaults(faults)}`,
    };
  }

  if (args.operationalMode === 'ACTIVE') {
    if (args.elapsedMs >= 1200) {
      return { done: true, kind: 'ok', message: 'Motors ACTIVE.' };
    }
    return { done: false, kind: 'pending', message: 'Enable accepted — holding ACTIVE…' };
  }

  // Gateway ACK is not enough: Davout may enable then immediately disable_all on hard limits.
  if (args.elapsedMs >= 700 && args.operationalMode === 'DISABLED') {
    return {
      done: true,
      kind: 'error',
      message: diagnoseEnableDisabledTrip(args.robotState, args.config),
    };
  }

  if (args.elapsedMs >= 4000) {
    return {
      done: true,
      kind: 'error',
      message: `Enable did not stay ACTIVE (mode: ${args.operationalMode ?? 'unknown'}).`,
    };
  }

  return {
    done: false,
    kind: 'pending',
    message: 'Enable queued — waiting for ACTIVE…',
  };
}
