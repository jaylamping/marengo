/** Shared Auto Learn wire types (Consul + BFF). */

export type AutoLearnStage = 'crawl' | 'walk' | 'run';

export type TeachKind = 'replace-native-wave' | 'replace-program';

export type StageEnvelope = {
  stage: AutoLearnStage;
  maxStepRomFraction: number;
  minSegmentDurationSec: number;
  minCadenceScale: number;
  maxSpeedMultiplier: number;
  minSettleDwellSec: number;
  defaultCadenceScale: number;
  defaultSpeedMultiplier: number;
  defaultSettleDwellSec: number;
};

export type AutoLearnLandmark = {
  id: string;
  label: string;
  tSec: number;
  q: Record<string, number>;
  included: boolean;
};

export type AutoLearnJoint = {
  name: string;
  positionRad: number;
  velocityRadS: number;
  torqueNm: number;
  tempC: number;
  positionLowerRad: number;
  positionUpperRad: number;
  velocityMaxRadS: number;
  torqueLimitNm: number;
};

export type AutoLearnLogContext = {
  attachedAtMs: number;
  truncated: boolean;
  sinceMs: number | null;
  summaryText: string;
};

export type AutoLearnBasePreset = {
  name: string;
  description: string;
  joints: string[];
  teachKind: TeachKind;
  keyframes: Record<string, { targetRad: number; durationSec: number }[]>;
  nativeWave?: {
    joint: string;
    minRad: number;
    maxRad: number;
    cycles: number;
    halfPeriodSec: number;
  };
};

export type AutoLearnRequest = {
  presetId: string;
  stage: AutoLearnStage;
  intent: string;
  operatorFeedback: string | null;
  joints: AutoLearnJoint[];
  livePositions: Record<string, number>;
  priorLandmarks: AutoLearnLandmark[] | null;
  priorDescription: string | null;
  logContext: AutoLearnLogContext | null;
  base: AutoLearnBasePreset;
};

export type AutoLearnResponse = {
  stage: AutoLearnStage;
  description: string;
  landmarks: AutoLearnLandmark[];
  cadenceScale: number;
  settleDwellSec: number;
  speedMultiplier: number;
  source: 'auto_learn';
};

export type AssertFailure = {
  code: string;
  message: string;
};

export type AssertResult =
  | { ok: true }
  | { ok: false; failures: AssertFailure[] };

export const OPERATOR_FEEDBACK_MAX_BYTES = 2048;
export const LOG_CONTEXT_MAX_BYTES = 8 * 1024;
export const LOG_CONTEXT_MAX_LINES = 80;

export const STAGE_ORDER: AutoLearnStage[] = ['crawl', 'walk', 'run'];

export function nextStage(stage: AutoLearnStage): AutoLearnStage | null {
  const i = STAGE_ORDER.indexOf(stage);
  if (i < 0 || i >= STAGE_ORDER.length - 1) return null;
  return STAGE_ORDER[i + 1]!;
}
