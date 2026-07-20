/** Pure helpers for Subsystems actuator Set Limits listening sessions. */

export type LimitListenGate = {
  connected: boolean;
  /** Davout/enable label; ACTIVE means torque is on and will fight a manual sweep. */
  operationalMode: string | null;
};

export type RunningBounds = {
  min: number;
  max: number;
  sampleCount: number;
  lastPosition: number | null;
};

export function emptyBounds(): RunningBounds {
  return {
    min: Number.POSITIVE_INFINITY,
    max: Number.NEGATIVE_INFINITY,
    sampleCount: 0,
    lastPosition: null,
  };
}

export function foldPosition(
  bounds: RunningBounds,
  position: number,
): RunningBounds {
  if (!Number.isFinite(position)) {
    return bounds;
  }
  if (bounds.sampleCount === 0) {
    return {
      min: position,
      max: position,
      sampleCount: 1,
      lastPosition: position,
    };
  }
  return {
    min: Math.min(bounds.min, position),
    max: Math.max(bounds.max, position),
    sampleCount: bounds.sampleCount + 1,
    lastPosition: position,
  };
}

/** Format measured limits like inventory Range strings (e.g. ±1.57 or −0.90–3.17). */
export function formatJointRange(min: number, max: number): string {
  if (!Number.isFinite(min) || !Number.isFinite(max)) {
    return '—';
  }
  const lo = Math.min(min, max);
  const hi = Math.max(min, max);
  const span = hi - lo;
  const mid = (hi + lo) / 2;
  const tol = Math.max(span * 0.02, 0.01);
  if (Math.abs(mid) <= tol && Math.abs(Math.abs(lo) - Math.abs(hi)) <= tol) {
    const mag = Math.max(Math.abs(lo), Math.abs(hi));
    return `±${mag.toFixed(2)}`;
  }
  const minus = '\u2212';
  const loStr = lo < 0 ? `${minus}${Math.abs(lo).toFixed(2)}` : lo.toFixed(2);
  return `${loStr}\u2013${hi.toFixed(2)}`;
}

/** Minimum span (rad) before a listen session can propose Apply Limits. */
export const MIN_PROPOSAL_SPAN_RAD = 0.05;

/** Minimum distinct samples before proposing a range. */
export const MIN_PROPOSAL_SAMPLES = 5;

export function proposedRangeFromBounds(bounds: RunningBounds): string | null {
  if (
    bounds.sampleCount < MIN_PROPOSAL_SAMPLES ||
    !Number.isFinite(bounds.min) ||
    !Number.isFinite(bounds.max)
  ) {
    return null;
  }
  if (bounds.max - bounds.min < MIN_PROPOSAL_SPAN_RAD) {
    return null;
  }
  return formatJointRange(bounds.min, bounds.max);
}

/**
 * Live listen is free-drive: Chappe connected and motors known non-ACTIVE.
 * Pi must publish live joint positions while limp (bench
 * `active_reporting_diagnostics` + Davout type-24 when not ACTIVE).
 * Mirror Set Zero: wait for operationalMode before starting (null is not safe).
 * Operator supports the assembly; no GravityComp / position hold.
 */
export function canStartLimitListen(gate: LimitListenGate): boolean {
  return (
    gate.connected &&
    gate.operationalMode !== null &&
    gate.operationalMode !== 'ACTIVE'
  );
}

export function limitListenBlockReason(gate: LimitListenGate): string | null {
  if (canStartLimitListen(gate)) {
    return null;
  }
  if (!gate.connected) {
    return 'Connect Chappe to listen.';
  }
  if (gate.operationalMode === null) {
    return 'Waiting for operational mode…';
  }
  if (gate.operationalMode === 'ACTIVE') {
    return 'Disable motors first — ACTIVE (hold / GravityComp) fights manual sweeps.';
  }
  return 'Cannot start listening.';
}
