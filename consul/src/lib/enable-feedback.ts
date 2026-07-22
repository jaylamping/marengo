import type {
  ActuatorLimitSnapshot,
  Fault,
  RobotState,
  SafetyState,
} from '@/gen/marengo/v1/marengo_pb';
import type { ConfigSnapshotDto } from '@/lib/config-api';
import type { OperationalModeLabel } from '@/state/robotStore';
import { liveJointEnvelope } from '@/state/actuatorStore';

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

export type JointPoseHint = {
  name: string;
  position: number;
  lower: number;
  upper: number;
};

type HardBounds = { lower: number; upper: number };

function hardBoundsForJoint(
  joint: string,
  limitSnapshot: ActuatorLimitSnapshot | null | undefined,
  config: ConfigSnapshotDto | null | undefined,
): HardBounds | null {
  const envelope = liveJointEnvelope(joint, limitSnapshot ?? null);
  if (envelope) {
    return { lower: envelope.hardLowerRad, upper: envelope.hardUpperRad };
  }
  const bench = config?.motors?.find((m) => m.joint === joint)?.bench;
  if (!bench) {
    return null;
  }
  return {
    lower: bench.position_lower_rad,
    upper: bench.position_upper_rad,
  };
}

/**
 * Compare live poses to Davout hard envelope (ActuatorLimitSnapshot).
 * Falls back to motors.yaml bench only when the live snapshot is missing.
 */
export function diagnoseEnableDisabledTrip(
  robotState: RobotState | null | undefined,
  config: ConfigSnapshotDto | null | undefined,
  limitSnapshot?: ActuatorLimitSnapshot | null,
): string {
  const joints = robotState?.joints ?? [];
  if (joints.length === 0) {
    return 'Enable posted, but motors returned to DISABLED (hard-limit / safety trip — check Pi journal).';
  }

  const outsideHard: JointPoseHint[] = [];
  const withinHard: JointPoseHint[] = [];
  let usedLiveSnapshot = false;

  for (const j of joints) {
    const bounds = hardBoundsForJoint(j.name, limitSnapshot, config);
    if (!bounds) continue;
    if (liveJointEnvelope(j.name, limitSnapshot ?? null)) {
      usedLiveSnapshot = true;
    }
    const hint: JointPoseHint = {
      name: j.name,
      position: j.position,
      lower: bounds.lower,
      upper: bounds.upper,
    };
    if (j.position < bounds.lower || j.position > bounds.upper) {
      outsideHard.push(hint);
    } else {
      withinHard.push(hint);
    }
  }

  if (outsideHard.length > 0) {
    const source = usedLiveSnapshot ? 'Davout hard' : 'motors bench';
    const detail = outsideHard
      .map(
        (h) =>
          `${h.name} at ${h.position.toFixed(3)} outside ${source} [${h.lower.toFixed(2)}, ${h.upper.toFixed(2)}]`,
      )
      .join(' · ');
    return `Enable posted, but motors returned to DISABLED — ${detail}.`;
  }

  if (withinHard.length > 0) {
    const detail = withinHard
      .map((h) => `${h.name}=${h.position.toFixed(3)}`)
      .join(', ');
    return (
      `Enable posted, but motors returned to DISABLED — poses within hard envelope (${detail}). ` +
      `Check Pi journal (CAN ENOBUFS / txqueuelen, watchdog, or other safety trip).`
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
  limitSnapshot?: ActuatorLimitSnapshot | null;
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
      message: diagnoseEnableDisabledTrip(
        args.robotState,
        args.config,
        args.limitSnapshot,
      ),
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
