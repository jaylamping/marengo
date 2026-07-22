import { z } from 'zod';
import {
  applyStageSeedDefaults,
  assertAutoLearnResponse,
  normalizeOperatorFeedback,
} from '../shared/asserts';
import type { AutoLearnRequest, AutoLearnResponse } from '../shared/types';
import { extractJsonObject } from './prompt';

const stageSchema = z.enum(['crawl', 'walk', 'run']);

const landmarkSchema = z.object({
  id: z.string().min(1),
  label: z.string(),
  tSec: z.number().finite(),
  q: z.record(z.string(), z.number().finite()),
  included: z.boolean(),
});

const responseSchema = z.object({
  stage: stageSchema,
  description: z.string(),
  landmarks: z.array(landmarkSchema).min(1),
  cadenceScale: z.number().finite().optional(),
  settleDwellSec: z.number().finite().optional(),
  speedMultiplier: z.number().finite().optional(),
  source: z.literal('auto_learn').optional(),
});

const jointSchema = z.object({
  name: z.string(),
  positionRad: z.number().finite(),
  velocityRadS: z.number().finite(),
  torqueNm: z.number().finite(),
  tempC: z.number().finite(),
  positionLowerRad: z.number().finite(),
  positionUpperRad: z.number().finite(),
  velocityMaxRadS: z.number().finite().positive(),
  torqueLimitNm: z.number().finite().positive(),
});

const requestSchema = z.object({
  presetId: z.string().min(1),
  stage: stageSchema,
  intent: z.string(),
  operatorFeedback: z.string().nullable(),
  joints: z.array(jointSchema).min(1),
  livePositions: z.record(z.string(), z.number().finite()),
  priorLandmarks: z.array(landmarkSchema).nullable(),
  priorDescription: z.string().nullable(),
  logContext: z
    .object({
      attachedAtMs: z.number().finite(),
      truncated: z.boolean(),
      sinceMs: z.number().finite().nullable(),
      summaryText: z.string().max(8 * 1024),
    })
    .nullable(),
  base: z.object({
    name: z.string(),
    description: z.string(),
    movementBrief: z.string().min(1),
    joints: z.array(z.string()).min(1),
    teachKind: z.enum(['replace-native-wave', 'replace-program']),
    keyframes: z.record(
      z.string(),
      z.array(
        z.object({
          targetRad: z.number().finite(),
          durationSec: z.number().finite(),
        }),
      ),
    ),
    nativeWave: z
      .object({
        joint: z.string(),
        minRad: z.number().finite(),
        maxRad: z.number().finite(),
        cycles: z.number().finite(),
        halfPeriodSec: z.number().finite(),
      })
      .optional(),
  }),
});

export function parseAutoLearnRequest(body: unknown): AutoLearnRequest {
  const parsed = requestSchema.parse(body);
  return {
    ...parsed,
    operatorFeedback: normalizeOperatorFeedback(parsed.operatorFeedback),
  };
}

export function parseAndAssertResponse(
  request: AutoLearnRequest,
  modelText: string,
):
  | { ok: true; response: AutoLearnResponse }
  | { ok: false; failures: { code: string; message: string }[] } {
  let raw: unknown;
  try {
    raw = extractJsonObject(modelText);
  } catch (err) {
    return {
      ok: false,
      failures: [
        {
          code: 'json_parse',
          message: err instanceof Error ? err.message : 'json parse failed',
        },
      ],
    };
  }

  const parsed = responseSchema.safeParse(raw);
  if (!parsed.success) {
    return {
      ok: false,
      failures: parsed.error.issues.map((i) => ({
        code: 'schema',
        message: `${i.path.join('.')}: ${i.message}`,
      })),
    };
  }

  const seeds = applyStageSeedDefaults(request.stage, {
    cadenceScale: parsed.data.cadenceScale,
    settleDwellSec: parsed.data.settleDwellSec,
    speedMultiplier: parsed.data.speedMultiplier,
  });

  const response: AutoLearnResponse = {
    stage: parsed.data.stage,
    description: parsed.data.description,
    landmarks: parsed.data.landmarks,
    ...seeds,
    source: 'auto_learn',
  };

  const asserted = assertAutoLearnResponse(request, response);
  if (!asserted.ok) {
    return { ok: false, failures: asserted.failures };
  }
  return { ok: true, response };
}
