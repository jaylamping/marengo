import { mkdirSync } from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { Agent } from '@cursor/sdk';
import { buildAutoLearnPrompt } from './prompt';
import type { AutoLearnRequest } from '../shared/types';

const here = path.dirname(fileURLToPath(import.meta.url));
const PI_DEFAULT_CWD = '/opt/marengo/var/auto-learn-cwd';

export type PromptFn = (prompt: string) => Promise<string>;

/**
 * Resolve Agent.prompt local cwd:
 * 1. `MARENGO_AUTO_LEARN_CWD` when set (must be creatable)
 * 2. else `/opt/marengo/var/auto-learn-cwd` when creatable (Pi default)
 * 3. else package-relative `schema-cwd` (local / tests)
 */
export function resolveAutoLearnCwd(): string {
  const fromEnv = process.env.MARENGO_AUTO_LEARN_CWD?.trim();
  if (fromEnv) {
    mkdirSync(fromEnv, { recursive: true });
    return path.resolve(fromEnv);
  }
  try {
    mkdirSync(PI_DEFAULT_CWD, { recursive: true });
    return PI_DEFAULT_CWD;
  } catch {
    // From src/ → ../schema-cwd; from bundled dist/server.js → ../schema-cwd
    const schemaDir = path.resolve(here, '../schema-cwd');
    mkdirSync(schemaDir, { recursive: true });
    return schemaDir;
  }
}

export function defaultPromptFn(): PromptFn {
  const cwd = resolveAutoLearnCwd();
  return async (prompt: string) => {
    const apiKey = process.env.CURSOR_API_KEY?.trim();
    if (!apiKey) {
      throw new Error('CURSOR_API_KEY is not set');
    }
    const result = await Agent.prompt(prompt, {
      apiKey,
      model: { id: 'composer-2.5' },
      local: { cwd },
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
