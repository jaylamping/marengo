import {
  FaultSeverity,
  type ActuatorLimitSnapshot,
  type Fault,
  type RobotState,
  type SafetyState,
} from '@/gen/marengo/v1/marengo_pb';
import type { ConfigSnapshotDto } from '@/lib/config-api';
import type { OperationalModeLabel } from '@/state/robotStore';
import { liveJointEnvelope } from '@/state/actuatorStore';

export function formatSafetyFaults(faults: Fault[]): string {
  if (faults.length === 0) return '';
  return faults
    .map((f) => {
      const severity =
        f.severity === FaultSeverity.ESTOP
          ? 'E-STOP '
          : f.severity === FaultSeverity.WARNING
            ? 'warning '
            : '';
      const joint = f.joint?.trim() ? `${f.joint}: ` : '';
      const code = f.code?.trim() ? `[${f.code}] ` : '';
      return `${severity}${joint}${code}${f.message || 'fault'}`.trim();
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
 * Pi-side bounds an Enable may legitimately take before Davout reports ACTIVE.
 * `ENABLE_COMPLETION_TIMEOUT` (berthier, 2 s) already covers a target held for
 * its post-SetZero quiet (800 ms) and the one-per-interface Enable stagger.
 */
const PI_ENABLE_COMPLETION_TIMEOUT_MS = 2000;
/** The gravity saturation preflight runs first, about 0.2 s on the Pi. */
const GRAVITY_PREFLIGHT_MS = 500;
/** IPC, the 25 Hz SafetyState publish and the gateway stream. */
const TELEMETRY_SLACK_MS = 1500;

/**
 * Never-ACTIVE Enable is reported as failed only after this long. A DISABLED
 * mode before then is the normal pre-enable state, not a trip.
 */
export const ENABLE_OUTCOME_TIMEOUT_MS =
  PI_ENABLE_COMPLETION_TIMEOUT_MS + GRAVITY_PREFLIGHT_MS + TELEMETRY_SLACK_MS;

/** ACTIVE must hold this long before the Enable is confirmed. */
export const ENABLE_ACTIVE_HOLD_MS = 1000;

/**
 * Interpret post-Enable telemetry for operator-facing status.
 *
 * Failure is reported only on evidence: an E-stop or fault, a published
 * `enable refused`, a drop to DISABLED after ACTIVE was seen, or no ACTIVE
 * within `ENABLE_OUTCOME_TIMEOUT_MS`. DISABLED alone, early, is still pending.
 */
export function interpretPostEnableWatch(args: {
  elapsedMs: number;
  operationalMode: OperationalModeLabel;
  safetyState: SafetyState | null;
  robotState?: RobotState | null;
  config?: ConfigSnapshotDto | null;
  limitSnapshot?: ActuatorLimitSnapshot | null;
  /** How long ACTIVE has been seen since the Enable; null if never. */
  activeForMs?: number | null;
  /** A Pi refusal of this Enable (`robot/audit/action`), if one arrived. */
  enableRefusal?: string | null;
}): { done: boolean; message: string | null; kind: 'ok' | 'error' | 'pending' } {
  if (args.safetyState?.hardwareEstopAsserted) {
    return {
      done: true,
      kind: 'error',
      message: 'Enable refused — hardware E-stop asserted.',
    };
  }
  const faults = args.safetyState?.activeFaults ?? [];
  if (faults.length > 0) {
    return {
      done: true,
      kind: 'error',
      message: `Enable tripped safety — ${formatSafetyFaults(faults)}`,
    };
  }
  if (args.enableRefusal) {
    return {
      done: true,
      kind: 'error',
      message: `Enable refused by the Pi — ${args.enableRefusal}`,
    };
  }

  const activeForMs = args.activeForMs ?? null;
  if (args.operationalMode === 'ACTIVE') {
    if ((activeForMs ?? 0) >= ENABLE_ACTIVE_HOLD_MS) {
      return { done: true, kind: 'ok', message: 'Motors ACTIVE.' };
    }
    return { done: false, kind: 'pending', message: 'Enable accepted — holding ACTIVE…' };
  }

  // Davout may enable then immediately disable_all on hard limits: only a drop
  // after ACTIVE was seen is a trip.
  if (activeForMs !== null && args.operationalMode === 'DISABLED') {
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

  if (args.elapsedMs >= ENABLE_OUTCOME_TIMEOUT_MS) {
    const mode = args.operationalMode ?? 'unknown';
    return {
      done: true,
      kind: 'error',
      message:
        activeForMs === null
          ? `Enable did not reach ACTIVE within ${(ENABLE_OUTCOME_TIMEOUT_MS / 1000).toFixed(1)} s ` +
            `(mode: ${mode}) — check the Pi journal for "enable refused" or a safety trip.`
          : `Enable did not stay ACTIVE (mode: ${mode}).`,
    };
  }

  return {
    done: false,
    kind: 'pending',
    message: 'Enable queued — waiting for ACTIVE…',
  };
}
