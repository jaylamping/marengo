/** Teach-record: sample buffer → landmark times → full-q snapshots. */

export interface TeachSample {
  tMs: number;
  q: Record<string, number>;
}

export interface TeachLandmark {
  id: string;
  label: string;
  tSec: number;
  q: Record<string, number>;
  included: boolean;
}

export interface LandmarkProposal {
  label: string;
  tSec: number;
}

const EPS = 1e-4;

function jointSeries(samples: TeachSample[], joint: string): { tSec: number; v: number }[] {
  const out: { tSec: number; v: number }[] = [];
  const t0 = samples[0]?.tMs ?? 0;
  for (const s of samples) {
    const v = s.q[joint];
    if (typeof v === 'number' && Number.isFinite(v)) {
      out.push({ tSec: (s.tMs - t0) / 1000, v });
    }
  }
  return out;
}

/** Local extrema times for one joint (noise-tolerant; dense Chappe OK). */
export function proposeExtremaTimes(
  samples: TeachSample[],
  joint: string,
  opts?: { minSeparationSec?: number; minProminence?: number }
): number[] {
  const series = jointSeries(samples, joint);
  if (series.length < 5) return [];

  const minSep = opts?.minSeparationSec ?? 0.35;
  const minProm = opts?.minProminence ?? 0.04;
  const halfWin = 8;
  const times: number[] = [];

  for (let i = 1; i < series.length - 1; i++) {
    const b = series[i - 1].v;
    const c = series[i].v;
    const d = series[i + 1].v;
    const isMax = c >= b && c > d;
    const isMin = c <= b && c < d;
    if (!isMax && !isMin) continue;

    const lo = Math.max(0, i - halfWin);
    const hi = Math.min(series.length, i + halfWin + 1);
    let localMin = Infinity;
    let localMax = -Infinity;
    for (let j = lo; j < hi; j++) {
      localMin = Math.min(localMin, series[j].v);
      localMax = Math.max(localMax, series[j].v);
    }
    const prom = isMax ? c - localMin : localMax - c;
    if (prom < minProm) continue;

    const t = series[i].tSec;
    if (times.length > 0 && t - times[times.length - 1] < minSep) continue;
    times.push(t);
  }
  return times;
}

/** Dwell: joint stays near value for at least holdSec. Returns mid-dwell time. */
export function proposeDwellTime(
  samples: TeachSample[],
  joint: string,
  nearRad: number,
  holdSec: number,
  tolRad = 0.08
): number | null {
  const series = jointSeries(samples, joint);
  if (series.length === 0) return null;

  let runStart: number | null = null;
  for (let i = 0; i < series.length; i++) {
    const near = Math.abs(series[i].v - nearRad) <= tolRad;
    if (near) {
      if (runStart === null) runStart = series[i].tSec;
      const dur = series[i].tSec - runStart;
      if (dur >= holdSec) {
        return runStart + dur / 2;
      }
    } else {
      runStart = null;
    }
  }
  return null;
}

/** Snapshot all wired joints at the sample closest to tSec. */
export function snapshotAtTime(
  samples: TeachSample[],
  joints: string[],
  tSec: number
): Record<string, number> | null {
  if (samples.length === 0) return null;
  const t0 = samples[0].tMs;
  let best = samples[0];
  let bestDt = Infinity;
  for (const s of samples) {
    const dt = Math.abs((s.tMs - t0) / 1000 - tSec);
    if (dt < bestDt) {
      bestDt = dt;
      best = s;
    }
  }
  const q: Record<string, number> = {};
  for (const j of joints) {
    const v = best.q[j];
    if (typeof v !== 'number' || !Number.isFinite(v)) return null;
    q[j] = v;
  }
  return q;
}

/**
 * Propose landmark *times* only, then snapshot full q at each accepted time.
 * Never stitches independent per-joint extrema into invented poses.
 */
export function extractLandmarks(
  samples: TeachSample[],
  joints: string[],
  opts?: {
    pitchJoint?: string;
    rollJoint?: string;
    yawJoint?: string;
  }
): TeachLandmark[] {
  if (samples.length < 8 || joints.length === 0) return [];
  // Fail closed: dwell-only / noise buffers must not produce Apply-able landmarks.
  if (!samplesHaveMotion(samples, joints)) return [];

  const pitch = opts?.pitchJoint ?? 'right_shoulder_pitch';
  const roll = opts?.rollJoint ?? 'right_shoulder_roll';
  const yaw = opts?.yawJoint ?? 'right_upper_arm_yaw';

  const proposals: LandmarkProposal[] = [];

  const home = proposeDwellTime(samples, pitch, 0, 0.4, 0.12);
  if (home !== null) {
    proposals.push({ label: 'home', tSec: home });
  }

  const pitchSeries = jointSeries(samples, pitch);
  const pitchMax = pitchSeries.reduce(
    (best, p) => (p.v > best.v ? p : best),
    pitchSeries[0] ?? { tSec: 0, v: -Infinity }
  );
  if (pitchMax && Number.isFinite(pitchMax.v) && pitchMax.v > 0.5) {
    const upright = proposeDwellTime(samples, pitch, pitchMax.v, 0.25, 0.15);
    proposals.push({
      label: 'upright',
      tSec: upright ?? pitchMax.tSec,
    });
  }

  for (const t of proposeExtremaTimes(samples, roll)) {
    proposals.push({ label: 'roll_extrema', tSec: t });
  }
  if (joints.includes(yaw)) {
    for (const t of proposeExtremaTimes(samples, yaw)) {
      proposals.push({ label: 'yaw_extrema', tSec: t });
    }
  }

  // Dedupe by time proximity; keep first label.
  proposals.sort((a, b) => a.tSec - b.tSec);
  const accepted: LandmarkProposal[] = [];
  for (const p of proposals) {
    if (accepted.some((a) => Math.abs(a.tSec - p.tSec) < 0.2)) continue;
    accepted.push(p);
  }

  const landmarks: TeachLandmark[] = [];
  for (let i = 0; i < accepted.length; i++) {
    const p = accepted[i];
    const q = snapshotAtTime(samples, joints, p.tSec);
    if (!q) continue;
    landmarks.push({
      id: `lm-${i}-${p.label}`,
      label: p.label,
      tSec: p.tSec,
      q,
      included: true,
    });
  }
  return landmarks;
}

export function includedLandmarks(landmarks: TeachLandmark[]): TeachLandmark[] {
  return landmarks.filter((l) => l.included);
}

/** Fail-closed: Apply requires at least two included full-q landmarks. */
export function canApplyLandmarks(landmarks: TeachLandmark[]): boolean {
  return includedLandmarks(landmarks).length >= 2;
}

export function samplesHaveMotion(samples: TeachSample[], joints: string[], minSpan = 0.05): boolean {
  for (const j of joints) {
    let min = Infinity;
    let max = -Infinity;
    for (const s of samples) {
      const v = s.q[j];
      if (typeof v !== 'number') continue;
      min = Math.min(min, v);
      max = Math.max(max, v);
    }
    if (max - min > minSpan + EPS) return true;
  }
  return false;
}
