import type { AutoLearnStage, StageEnvelope } from './types';

export const STAGE_ENVELOPES: Record<AutoLearnStage, StageEnvelope> = {
  crawl: {
    stage: 'crawl',
    maxStepRomFraction: 0.15,
    minSegmentDurationSec: 2.0,
    minCadenceScale: 1.5,
    maxSpeedMultiplier: 0.35,
    minSettleDwellSec: 0.4,
    defaultCadenceScale: 1.5,
    defaultSpeedMultiplier: 0.35,
    defaultSettleDwellSec: 0.4,
  },
  walk: {
    stage: 'walk',
    maxStepRomFraction: 0.4,
    minSegmentDurationSec: 1.0,
    minCadenceScale: 1.0,
    maxSpeedMultiplier: 0.6,
    minSettleDwellSec: 0.25,
    defaultCadenceScale: 1.0,
    defaultSpeedMultiplier: 0.6,
    defaultSettleDwellSec: 0.25,
  },
  run: {
    stage: 'run',
    maxStepRomFraction: 0.7,
    minSegmentDurationSec: 0.5,
    minCadenceScale: 0.75,
    maxSpeedMultiplier: 0.85,
    minSettleDwellSec: 0.15,
    defaultCadenceScale: 0.75,
    defaultSpeedMultiplier: 0.85,
    defaultSettleDwellSec: 0.15,
  },
};

export function stageEnvelope(stage: AutoLearnStage): StageEnvelope {
  return STAGE_ENVELOPES[stage];
}

/** Max |Δq| for a joint from bench ROM and stage fraction. */
export function maxStepRad(
  positionLowerRad: number,
  positionUpperRad: number,
  romFraction: number,
): number {
  const rom = Math.max(0, positionUpperRad - positionLowerRad);
  return rom * romFraction;
}
