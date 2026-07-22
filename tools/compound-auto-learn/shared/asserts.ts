import { maxStepRad, stageEnvelope } from './envelopes';
import {
  OPERATOR_FEEDBACK_MAX_BYTES,
  type AssertFailure,
  type AssertResult,
  type AutoLearnJoint,
  type AutoLearnLandmark,
  type AutoLearnRequest,
  type AutoLearnResponse,
  type AutoLearnStage,
} from './types';

function fail(code: string, message: string): AssertFailure {
  return { code, message };
}

function isFiniteNumber(n: unknown): n is number {
  return typeof n === 'number' && Number.isFinite(n);
}

export function includedLandmarks(
  landmarks: AutoLearnLandmark[],
): AutoLearnLandmark[] {
  return landmarks.filter((l) => l.included);
}

/** Match teach-transit approach duration for first landmark. */
export function approachDurationSec(
  cadenceScale: number,
  settleDwellSec: number,
): number {
  const scale = Math.max(0.25, cadenceScale);
  const dwell = Math.max(0, settleDwellSec);
  return Math.max(0.5, 0.5 * scale + dwell);
}

/** Match teach-transit segment durations (without speedMultiplier). */
export function materializeSegmentDurationsSec(
  landmarks: AutoLearnLandmark[],
  cadenceScale: number,
  settleDwellSec: number,
): number[] | null {
  const included = includedLandmarks(landmarks);
  if (included.length < 2) return null;
  const scale = Math.max(0.25, cadenceScale);
  const dwell = Math.max(0, settleDwellSec);
  const out: number[] = [approachDurationSec(cadenceScale, settleDwellSec)];
  for (let i = 1; i < included.length; i++) {
    const lm = included[i]!;
    const prevT = included[i - 1]!.tSec;
    out.push(Math.max(0.15, (lm.tSec - prevT) * scale) + dwell);
  }
  return out;
}

export function jointLimitsByName(
  joints: AutoLearnJoint[],
): Map<string, AutoLearnJoint> {
  return new Map(joints.map((j) => [j.name, j]));
}

export function assertCrawlFirst(
  stage: AutoLearnStage,
  priorLandmarks: AutoLearnLandmark[] | null,
): AssertResult {
  if (priorLandmarks == null && stage !== 'crawl') {
    return {
      ok: false,
      failures: [
        fail(
          'crawl_first',
          `stage=${stage} requires priorLandmarks; first generation must be crawl`,
        ),
      ],
    };
  }
  return { ok: true };
}

export function assertLandmarksShape(
  response: AutoLearnResponse,
  request: AutoLearnRequest,
): AssertResult {
  const failures: AssertFailure[] = [];
  if (response.stage !== request.stage) {
    failures.push(
      fail(
        'stage_mismatch',
        `response.stage=${response.stage} != request.stage=${request.stage}`,
      ),
    );
  }
  if (response.source !== 'auto_learn') {
    failures.push(fail('bad_source', 'response.source must be auto_learn'));
  }
  const env = stageEnvelope(request.stage);
  if (
    !isFiniteNumber(response.cadenceScale) ||
    response.cadenceScale < env.minCadenceScale
  ) {
    failures.push(
      fail(
        'cadence_floor',
        `cadenceScale=${response.cadenceScale} < min ${env.minCadenceScale}`,
      ),
    );
  }
  if (
    !isFiniteNumber(response.settleDwellSec) ||
    response.settleDwellSec < env.minSettleDwellSec
  ) {
    failures.push(
      fail(
        'dwell_floor',
        `settleDwellSec=${response.settleDwellSec} < min ${env.minSettleDwellSec}`,
      ),
    );
  }
  if (
    !isFiniteNumber(response.speedMultiplier) ||
    response.speedMultiplier > env.maxSpeedMultiplier ||
    response.speedMultiplier <= 0
  ) {
    failures.push(
      fail(
        'speed_ceiling',
        `speedMultiplier=${response.speedMultiplier} must be in (0, ${env.maxSpeedMultiplier}]`,
      ),
    );
  }

  const included = includedLandmarks(response.landmarks);
  const minIncluded =
    request.base.teachKind === 'replace-native-wave' ? 3 : 2;
  if (included.length < minIncluded) {
    failures.push(
      fail(
        'landmark_count',
        `need ≥${minIncluded} included landmarks for ${request.base.teachKind}, got ${included.length}`,
      ),
    );
  }

  let prevT = -Infinity;
  for (const lm of included) {
    if (!isFiniteNumber(lm.tSec)) {
      failures.push(fail('bad_tSec', `landmark ${lm.id} has non-finite tSec`));
      continue;
    }
    if (lm.tSec < prevT) {
      failures.push(
        fail('tSec_order', `landmark ${lm.id} tSec not monotonic`),
      );
    }
    prevT = lm.tSec;
    for (const joint of request.base.joints) {
      const q = lm.q[joint];
      if (!isFiniteNumber(q)) {
        failures.push(
          fail('missing_joint', `landmark ${lm.id} missing q[${joint}]`),
        );
      }
    }
  }

  return failures.length ? { ok: false, failures } : { ok: true };
}

function pushStepFailure(
  failures: AssertFailure[],
  code: string,
  label: string,
  jointName: string,
  from: number,
  to: number,
  lim: AutoLearnJoint,
  romFraction: number,
): void {
  const maxStep = maxStepRad(
    lim.positionLowerRad,
    lim.positionUpperRad,
    romFraction,
  );
  const dq = Math.abs(to - from);
  if (dq > maxStep + 1e-9) {
    failures.push(
      fail(
        code,
        `${label} |Δ${jointName}|=${dq.toFixed(4)} > maxStep ${maxStep.toFixed(4)}`,
      ),
    );
  }
}

export function assertPositionsAndSteps(
  landmarks: AutoLearnLandmark[],
  joints: AutoLearnJoint[],
  livePositions: Record<string, number>,
  stage: AutoLearnStage,
): AssertResult {
  const failures: AssertFailure[] = [];
  const env = stageEnvelope(stage);
  const byName = jointLimitsByName(joints);
  const included = includedLandmarks(landmarks);
  if (included.length === 0) {
    return { ok: false, failures: [fail('empty', 'no included landmarks')] };
  }

  for (const lm of included) {
    for (const [name, q] of Object.entries(lm.q)) {
      const lim = byName.get(name);
      if (!lim || !isFiniteNumber(q)) continue;
      if (q < lim.positionLowerRad || q > lim.positionUpperRad) {
        failures.push(
          fail(
            'position_limit',
            `${lm.id} ${name}=${q} outside [${lim.positionLowerRad}, ${lim.positionUpperRad}]`,
          ),
        );
      }
    }
  }

  const first = included[0]!;
  for (const joint of joints) {
    const lim = byName.get(joint.name);
    if (!lim) continue;
    const live = livePositions[joint.name];
    const q0 = first.q[joint.name];
    if (!isFiniteNumber(live) || !isFiniteNumber(q0)) continue;
    pushStepFailure(
      failures,
      'live_first_step',
      `live→${first.id}`,
      joint.name,
      live,
      q0,
      lim,
      env.maxStepRomFraction,
    );
  }

  for (let i = 1; i < included.length; i++) {
    const a = included[i - 1]!;
    const b = included[i]!;
    for (const joint of joints) {
      const lim = byName.get(joint.name);
      if (!lim) continue;
      const qa = a.q[joint.name];
      const qb = b.q[joint.name];
      if (!isFiniteNumber(qa) || !isFiniteNumber(qb)) continue;
      pushStepFailure(
        failures,
        'segment_step',
        `${a.id}→${b.id}`,
        joint.name,
        qa,
        qb,
        lim,
        env.maxStepRomFraction,
      );
    }
  }

  return failures.length ? { ok: false, failures } : { ok: true };
}

export function assertMaterializedSchedule(
  landmarks: AutoLearnLandmark[],
  cadenceScale: number,
  settleDwellSec: number,
  speedMultiplier: number,
  stage: AutoLearnStage,
): AssertResult {
  const env = stageEnvelope(stage);
  const durations = materializeSegmentDurationsSec(
    landmarks,
    cadenceScale,
    settleDwellSec,
  );
  if (!durations) {
    return {
      ok: false,
      failures: [fail('materialize', 'could not materialize segment durations')],
    };
  }
  const speed = Math.max(1e-6, speedMultiplier);
  const failures: AssertFailure[] = [];
  for (let i = 0; i < durations.length; i++) {
    const effective = durations[i]! / speed;
    if (effective + 1e-9 < env.minSegmentDurationSec) {
      failures.push(
        fail(
          'min_segment',
          `segment[${i}] effective ${effective.toFixed(3)}s < min ${env.minSegmentDurationSec}s`,
        ),
      );
    }
  }
  return failures.length ? { ok: false, failures } : { ok: true };
}

/** Full generate/Apply assert suite. */
export function assertAutoLearnResponse(
  request: AutoLearnRequest,
  response: AutoLearnResponse,
): AssertResult {
  const parts: AssertResult[] = [
    assertCrawlFirst(request.stage, request.priorLandmarks),
    assertLandmarksShape(response, request),
    assertPositionsAndSteps(
      response.landmarks,
      request.joints,
      request.livePositions,
      request.stage,
    ),
    assertMaterializedSchedule(
      response.landmarks,
      response.cadenceScale,
      response.settleDwellSec,
      response.speedMultiplier,
      request.stage,
    ),
  ];
  const failures = parts.flatMap((p) => (p.ok ? [] : p.failures));
  return failures.length ? { ok: false, failures } : { ok: true };
}

export function normalizeOperatorFeedback(
  raw: string | null | undefined,
): string | null {
  if (raw == null) return null;
  const trimmed = raw.trim();
  if (!trimmed) return null;
  const encoder = new TextEncoder();
  if (encoder.encode(trimmed).length <= OPERATOR_FEEDBACK_MAX_BYTES) {
    return trimmed;
  }
  let out = trimmed;
  while (
    encoder.encode(out).length > OPERATOR_FEEDBACK_MAX_BYTES &&
    out.length > 0
  ) {
    out = out.slice(0, -16);
  }
  return out || null;
}

export function applyStageSeedDefaults(
  stage: AutoLearnStage,
  partial: Partial<
    Pick<
      AutoLearnResponse,
      'cadenceScale' | 'settleDwellSec' | 'speedMultiplier'
    >
  >,
): Pick<
  AutoLearnResponse,
  'cadenceScale' | 'settleDwellSec' | 'speedMultiplier'
> {
  const env = stageEnvelope(stage);
  return {
    cadenceScale: partial.cadenceScale ?? env.defaultCadenceScale,
    settleDwellSec: partial.settleDwellSec ?? env.defaultSettleDwellSec,
    speedMultiplier: partial.speedMultiplier ?? env.defaultSpeedMultiplier,
  };
}
