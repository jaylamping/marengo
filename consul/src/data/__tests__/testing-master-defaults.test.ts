import { describe, expect, it } from 'vitest';
import { readFileSync } from 'node:fs';
import { resolve } from 'node:path';

import { PRESET_OPTIONS } from '@/components/dashboard/inventory/constants';
import { DEFAULT_OPERATOR_PROFILE } from '@/lib/bringup-presets';
import { robotInventory } from '@/data/robot-inventory';

const root = resolve(import.meta.dirname, '../../..');

function readSrc(rel: string): string {
  return readFileSync(resolve(root, 'src', rel), 'utf8');
}

describe('Testing / operator defaults — master inventory', () => {
  it('exports DEFAULT_OPERATOR_PROFILE as master', () => {
    expect(DEFAULT_OPERATOR_PROFILE).toBe('master');
  });

  it('purges bench_4dof from inventory preset column defaults', () => {
    const presets = robotInventory.map((item) => item.preset);
    expect(presets).not.toContain('bench_4dof');
    expect(presets.some((p) => p === 'arm_4dof_right')).toBe(false);
  });

  it('drops bench_4dof from inventory PRESET_OPTIONS', () => {
    expect(PRESET_OPTIONS.map((o) => o.value)).not.toContain('bench_4dof');
  });

  it('hooks and Testing panels default profile fallback to master', () => {
    const files = [
      'hooks/use-manual-movement-controller.ts',
      'hooks/use-compound-playback.ts',
      'hooks/use-auto-learn-controller.ts',
      'components/dashboard/testing/compound-test-panel.tsx',
    ];
    for (const rel of files) {
      const src = readSrc(rel);
      expect(src, rel).not.toMatch(/arm_4dof_right/);
      expect(src, rel).toMatch(/DEFAULT_OPERATOR_PROFILE|['"]master['"]/);
    }
  });
});
