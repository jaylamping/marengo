import type { CompoundTestPreset, Keyframe } from '@/data/compound-tests';
import { parseDeployRev } from '@/lib/host-debug-info';
import { overlayNeedsCalibrationAck } from '@/lib/teach-calibration';
import {
  canApplyLandmarks,
  includedLandmarks,
  type TeachLandmark,
} from '@/lib/teach-record';

export const TEACH_SESSION_VERSION = 1 as const;

/** Fingerprint for refuse-on-mismatch after redeploy / profile drift. */
export interface TeachFingerprint {
  profile: string;
  joints: string[];
  /** Deploy / config fingerprint (git rev or gateway config hash). */
  deployRev: string;
}

export interface TeachSession extends TeachFingerprint {
  version: typeof TEACH_SESSION_VERSION;
  presetId: string;
  landmarks: TeachLandmark[];
  /** Scales taught Δt → keyframe durationSec (cadence). Not Berthier speed. */
  cadenceScale: number;
  /** Extra settle dwell added after each landmark (seconds). */
  settleDwellSec: number;
  createdAtMs: number;
  /**
   * Calibration epoch at Apply. Soft-invalidate uses teachStore live epoch + ack
   * (set-zero), not this field alone — home does not bump.
   */
  calibrationEpoch: number;
}

export type TeachApplyError =
  | 'empty_landmarks'
  | 'version_mismatch'
  | 'fingerprint_mismatch'
  | 'preset_mismatch'
  | 'calibration_ack_required';

export function createTeachSession(
  fingerprint: TeachFingerprint,
  presetId: string,
  landmarks: TeachLandmark[],
  opts?: {
    cadenceScale?: number;
    settleDwellSec?: number;
    calibrationEpoch?: number;
  }
): TeachSession {
  return {
    version: TEACH_SESSION_VERSION,
    ...fingerprint,
    joints: [...fingerprint.joints],
    presetId,
    landmarks,
    cadenceScale: opts?.cadenceScale ?? 1,
    settleDwellSec: opts?.settleDwellSec ?? 0,
    createdAtMs: Date.now(),
    calibrationEpoch: opts?.calibrationEpoch ?? 0,
  };
}

/**
 * Shared deployRev/profile/joints fingerprint for Apply + Wave replay gates.
 * Prefer deployRev, but fall back to gitSha when deployRev is missing/empty
 * (proto3 string defaults to "" — `??` would not fall through).
 */
export function liveFingerprint(
  profile: string,
  joints: string[],
  build?: { deployRev?: string; gitSha?: string } | string | null
): TeachFingerprint {
  const raw =
    typeof build === 'string'
      ? build
      : (build?.deployRev || build?.gitSha || '');
  const cleaned = raw.trim();
  const deployRev = cleaned ? parseDeployRev(cleaned).rev : 'unknown';
  return { profile, joints: [...joints], deployRev: deployRev || 'unknown' };
}

export function fingerprintsMatch(a: TeachFingerprint, b: TeachFingerprint): boolean {
  if (a.profile !== b.profile) return false;
  // Fail closed until both sides have a concrete deploy rev (no unknown wildcard).
  if (a.deployRev === 'unknown' || b.deployRev === 'unknown') {
    return false;
  }
  if (a.deployRev !== b.deployRev) {
    return false;
  }
  if (a.joints.length !== b.joints.length) return false;
  for (let i = 0; i < a.joints.length; i++) {
    if (a.joints[i] !== b.joints[i]) return false;
  }
  return true;
}

/**
 * Cadence/dwell only: durationSec is when Consul posts the next endpoint.
 * Berthier still owns trajectory velocity/acceleration.
 * Does not multiply by compound runner speedMultiplier (caller applies once).
 */
export function landmarksToKeyframes(
  landmarks: TeachLandmark[],
  joints: string[],
  cadenceScale: number,
  settleDwellSec: number
): Record<string, Keyframe[]> | null {
  const included = includedLandmarks(landmarks);
  if (included.length < 2) return null;

  const scale = Math.max(0.25, cadenceScale);
  const dwell = Math.max(0, settleDwellSec);
  const keyframes: Record<string, Keyframe[]> = {};
  for (const j of joints) {
    keyframes[j] = [];
  }

  for (let i = 0; i < included.length; i++) {
    const lm = included[i];
    const prevT = i === 0 ? included[0].tSec : included[i - 1].tSec;
    const delta = Math.max(0.15, (lm.tSec - prevT) * scale) + (i === 0 ? 0 : dwell);
    // First landmark: short approach duration from current pose.
    const durationSec = i === 0 ? Math.max(0.5, 0.5 * scale + dwell) : delta;
    for (const j of joints) {
      const target = lm.q[j];
      if (typeof target !== 'number') return null;
      keyframes[j].push({ targetRad: target, durationSec });
    }
  }
  return keyframes;
}

/**
 * Build a Wave overlay: raise-once + looping extrema (no nativeWave).
 * First landmark is raise endpoint; remaining landmarks are the loop body.
 */
export function sessionToWaveOverlay(
  session: TeachSession,
  base: CompoundTestPreset,
  liveFingerprint: TeachFingerprint
): { ok: true; preset: CompoundTestPreset } | { ok: false; error: TeachApplyError } {
  if (session.version !== TEACH_SESSION_VERSION) {
    return { ok: false, error: 'version_mismatch' };
  }
  if (session.presetId !== base.id) {
    return { ok: false, error: 'preset_mismatch' };
  }
  if (!fingerprintsMatch(session, liveFingerprint)) {
    return { ok: false, error: 'fingerprint_mismatch' };
  }
  if (!canApplyLandmarks(session.landmarks)) {
    return { ok: false, error: 'empty_landmarks' };
  }

  const keyframes = landmarksToKeyframes(
    session.landmarks,
    session.joints,
    session.cadenceScale,
    session.settleDwellSec
  );
  if (!keyframes) {
    return { ok: false, error: 'empty_landmarks' };
  }

  const included = includedLandmarks(session.landmarks);
  // Need raise + ≥1 extrema landmark to loop without re-raise.
  const canLoopExtrema = included.length > 2;

  return {
    ok: true,
    preset: {
      ...base,
      description: `${base.description} [taught overlay]`,
      joints: [...session.joints],
      keyframes,
      loop: canLoopExtrema,
      advance: 'timed',
      nativeWave: undefined,
      loopFromSegment: canLoopExtrema ? 1 : undefined,
    },
  };
}

/** Merge shipped preset with applied overlay (overlay wins when present). */
export function resolveEffectivePreset(
  base: CompoundTestPreset,
  overlay: CompoundTestPreset | null
): CompoundTestPreset {
  return overlay ?? base;
}

/**
 * Refuse replay on hard fingerprint mismatch, or soft-invalidate when
 * calibration epoch advanced since last Ack/Apply (set-zero), until Ack.
 */
export function overlayReplayAllowed(
  session: TeachSession | undefined,
  live: TeachFingerprint,
  soft?: { liveCalibrationEpoch: number; ackedAtEpoch: number }
): { ok: true } | { ok: false; error: TeachApplyError } {
  if (!session) {
    return { ok: true };
  }
  if (session.version !== TEACH_SESSION_VERSION) {
    return { ok: false, error: 'version_mismatch' };
  }
  if (!fingerprintsMatch(session, live)) {
    return { ok: false, error: 'fingerprint_mismatch' };
  }
  if (
    soft &&
    overlayNeedsCalibrationAck(session, {
      liveCalibrationEpoch: soft.liveCalibrationEpoch,
      ackedAtEpoch: soft.ackedAtEpoch,
    })
  ) {
    return { ok: false, error: 'calibration_ack_required' };
  }
  return { ok: true };
}

export interface TeachOverlayGateInput {
  session: TeachSession;
  ackedAtEpoch: number;
}

/**
 * Single play-time resolver: materialize overlay from session landmarks, or fall
 * back to shipped base so a refused/stale overlay never bricks Wave.
 */
export function resolvePlayablePreset(
  base: CompoundTestPreset,
  entry: TeachOverlayGateInput | undefined,
  live: TeachFingerprint,
  liveCalibrationEpoch: number
): {
  preset: CompoundTestPreset;
  usingOverlay: boolean;
  /** Soft-cal / fingerprint issues — Wave still starts on shipped base. */
  warning: string | null;
} {
  if (!entry) {
    return { preset: base, usingOverlay: false, warning: null };
  }
  const soft = {
    liveCalibrationEpoch,
    ackedAtEpoch: entry.ackedAtEpoch,
  };
  const gate = overlayReplayAllowed(entry.session, live, soft);
  if (!gate.ok) {
    const warning =
      gate.error === 'calibration_ack_required'
        ? 'Taught overlay needs Acknowledge & keep (or Reset) — playing shipped Wave.'
        : `Taught overlay unavailable (${gate.error}) — playing shipped Wave.`;
    return { preset: base, usingOverlay: false, warning };
  }
  const built = sessionToWaveOverlay(entry.session, base, live);
  if (!built.ok) {
    return {
      preset: base,
      usingOverlay: false,
      warning: `Taught overlay unavailable (${built.error}) — playing shipped Wave.`,
    };
  }
  return { preset: built.preset, usingOverlay: true, warning: null };
}
