#!/usr/bin/env node
/**
 * Spike: load @cursor/sdk and run a trivial Agent.prompt when CURSOR_API_KEY is set.
 * Prints PASS/FAIL and exits non-zero on failure.
 */
import { mkdirSync } from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const here = path.dirname(fileURLToPath(import.meta.url));
const cwd = path.resolve(here, '../schema-cwd');

async function main() {
  const importOnly = process.env.SPIKE_IMPORT_ONLY === '1';
  let Agent;
  try {
    ({ Agent } = await import('@cursor/sdk'));
  } catch (err) {
    const message = err instanceof Error ? err.message : String(err);
    console.error(`FAIL: could not load @cursor/sdk: ${message}`);
    process.exit(1);
  }

  if (importOnly) {
    console.log('PASS: @cursor/sdk import ok (SPIKE_IMPORT_ONLY=1)');
    process.exit(0);
  }

  const apiKey = process.env.CURSOR_API_KEY?.trim();
  if (!apiKey) {
    console.error('FAIL: CURSOR_API_KEY is not set (set SPIKE_IMPORT_ONLY=1 to skip Agent.prompt)');
    process.exit(1);
  }

  mkdirSync(cwd, { recursive: true });

  try {
    const result = await Agent.prompt('Reply with exactly the single word: ok', {
      apiKey,
      model: { id: 'composer-2.5' },
      local: { cwd },
    });
    if (result.status === 'error') {
      console.error(`FAIL: Agent.prompt status=${result.status}`);
      process.exit(1);
    }
    console.log('PASS');
    process.exit(0);
  } catch (err) {
    const message = err instanceof Error ? err.message : String(err);
    console.error(`FAIL: ${message}`);
    process.exit(1);
  }
}

void main();
