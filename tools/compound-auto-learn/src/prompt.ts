import { stageEnvelope } from '../shared/envelopes';
import type { AutoLearnRequest } from '../shared/types';

export function buildAutoLearnPrompt(request: AutoLearnRequest): string {
  const env = stageEnvelope(request.stage);
  return [
    'You generate a Marengo compound-test teach overlay as JSON only.',
    'Do not edit files. Do not call tools. Final answer must be a single JSON object.',
    '',
    '## Stage envelope (hard)',
    JSON.stringify(env, null, 2),
    '',
    '## Joint telemetry and limits',
    JSON.stringify(request.joints, null, 2),
    '',
    '## Live positions (approach from here)',
    JSON.stringify(request.livePositions, null, 2),
    '',
    '## Base preset',
    JSON.stringify(request.base, null, 2),
    '',
    '## Prior landmarks',
    request.priorLandmarks
      ? JSON.stringify(
          {
            description: request.priorDescription,
            landmarks: request.priorLandmarks,
          },
          null,
          2,
        )
      : 'null (first generation — crawl only, small ROM)',
    '',
    '## Operator feedback (highest revision priority)',
    request.operatorFeedback ?? '(none)',
    '',
    '## Session logs (advisory only)',
    request.logContext?.summaryText ?? '(none)',
    '',
    '## Intent',
    request.intent,
    '',
    '## Response JSON schema',
    `{
  "stage": "${request.stage}",
  "description": string,
  "landmarks": [{ "id", "label", "tSec", "q": { <every base.joints name>: number }, "included": boolean }],
  "cadenceScale": number (>= ${env.minCadenceScale}),
  "settleDwellSec": number (>= ${env.minSettleDwellSec}),
  "speedMultiplier": number (in (0, ${env.maxSpeedMultiplier}]),
  "source": "auto_learn"
}`,
    '',
    'Rules:',
    `- stage must be exactly "${request.stage}".`,
    `- Every landmark q must stay inside position limits.`,
    `- |Δq| between live→first and consecutive included landmarks ≤ ${env.maxStepRomFraction} of each joint ROM.`,
    `- ${request.base.teachKind === 'replace-native-wave' ? '≥3 included landmarks (raise + ≥2 extrema).' : '≥2 included landmarks.'}`,
    '- Prefer slow conservative motion. Torque is advisory; keep motion gentle.',
    '- When operator feedback is present, prioritize it over expanding ROM.',
    '- Output JSON only, no markdown fences.',
  ].join('\n');
}

export function extractJsonObject(text: string): unknown {
  const trimmed = text.trim();
  try {
    return JSON.parse(trimmed);
  } catch {
    /* fall through */
  }
  const fenced = trimmed.match(/```(?:json)?\s*([\s\S]*?)```/i);
  if (fenced?.[1]) {
    return JSON.parse(fenced[1].trim());
  }
  const start = trimmed.indexOf('{');
  const end = trimmed.lastIndexOf('}');
  if (start >= 0 && end > start) {
    return JSON.parse(trimmed.slice(start, end + 1));
  }
  throw new Error('no JSON object in model output');
}
