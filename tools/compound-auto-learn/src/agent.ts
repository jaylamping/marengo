import { mkdirSync } from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { Agent } from '@cursor/sdk';
import { buildAutoLearnPrompt } from './prompt';
import type { AutoLearnRequest } from '../shared/types';

const here = path.dirname(fileURLToPath(import.meta.url));
const schemaDir = path.resolve(here, '../schema-cwd');

export type PromptFn = (prompt: string) => Promise<string>;

export function defaultPromptFn(): PromptFn {
  mkdirSync(schemaDir, { recursive: true });
  return async (prompt: string) => {
    const apiKey = process.env.CURSOR_API_KEY?.trim();
    if (!apiKey) {
      throw new Error('CURSOR_API_KEY is not set');
    }
    const result = await Agent.prompt(prompt, {
      apiKey,
      model: { id: 'composer-2.5' },
      local: { cwd: schemaDir },
    });
    if (result.status === 'error') {
      throw new Error(`Agent.prompt failed: ${result.status}`);
    }
    const text =
      typeof result.result === 'string'
        ? result.result
        : JSON.stringify(result.result ?? '');
    return text;
  };
}

export async function runAutoLearnPrompt(
  request: AutoLearnRequest,
  promptFn: PromptFn,
  repairHint?: string,
): Promise<string> {
  const prompt = repairHint
    ? `${buildAutoLearnPrompt(request)}\n\n## Previous attempt failed validation\n${repairHint}\nFix the JSON and respond with corrected JSON only.`
    : buildAutoLearnPrompt(request);
  return promptFn(prompt);
}
