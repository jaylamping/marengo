/**
 * Wire-gated Reference facet for Telemetry.
 * Never consults localStorage / actuatorZeroStore — Ready only from wire.
 */

export type TelemetryReferenceFacet = 'unknown' | 'ready' | 'not_ready';

/** Proto JointHomingState ordinals (reserved until codegen lands in PR3). */
export const JointHomingStateWire = {
  UNSPECIFIED: 0,
  UNHOMED: 1,
  HOMING: 2,
  VERIFIED: 3,
  FAULTED: 4,
} as const;

type JointLike = {
  name?: string;
  homingState?: unknown;
  homing_state?: unknown;
  /** Ignored — browser/local flags must not fabricate Ready. */
  zeroed?: unknown;
  ready?: unknown;
  [key: string]: unknown;
};

function rawHomingState(joint: JointLike): unknown {
  if (Object.prototype.hasOwnProperty.call(joint, 'homingState')) {
    return joint.homingState;
  }
  if (Object.prototype.hasOwnProperty.call(joint, 'homing_state')) {
    return joint.homing_state;
  }
  return undefined;
}

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

/**
 * Resolve Reference facet from optional wire `homing_state`.
 * Absent / UNSPECIFIED → unknown (old runtime gate).
 */
export function resolveTelemetryReferenceFacet(
  joint: JointLike | null | undefined,
): TelemetryReferenceFacet {
  if (joint == null) {
    return 'unknown';
  }
  const normalized = normalizeHoming(rawHomingState(joint));
  if (normalized === undefined) {
    return 'unknown';
  }
  if (
    normalized === JointHomingStateWire.UNSPECIFIED ||
    normalized === 'UNSPECIFIED' ||
    normalized === ''
  ) {
    return 'unknown';
  }
  if (
    normalized === JointHomingStateWire.VERIFIED ||
    normalized === 'VERIFIED'
  ) {
    return 'ready';
  }
  return 'not_ready';
}

export function telemetryReferenceFacetLabel(
  facet: TelemetryReferenceFacet,
): string {
  switch (facet) {
    case 'ready':
      return 'Ready';
    case 'not_ready':
      return 'Not ready';
    case 'unknown':
      return 'Unknown';
    default: {
      const _exhaustive: never = facet;
      return _exhaustive;
    }
  }
}
