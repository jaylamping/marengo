import { describe, expect, it } from 'vitest';

import { PRESET_OPTIONS } from '@/components/dashboard/inventory/constants';
import { robotInventory } from '@/data/robot-inventory';
import {
  DEFAULT_OPERATOR_PROFILE,
  PRESET_TO_PROFILE,
  PROFILE_TO_PRESET,
} from '@/lib/bringup-presets';

describe('Testing / operator defaults — master inventory', () => {
  it('exports DEFAULT_OPERATOR_PROFILE as master', () => {
    expect(DEFAULT_OPERATOR_PROFILE).toBe('master');
    expect(DEFAULT_OPERATOR_PROFILE).not.toBe('arm_4dof_right');
  });

  it('purges bench_4dof from inventory preset column defaults', () => {
    const presets = robotInventory.map((item) => item.preset);
    expect(presets).not.toContain('bench_4dof');
    expect(presets).not.toContain('arm_4dof_right');
  });

  it('drops bench_4dof from inventory PRESET_OPTIONS', () => {
    expect(PRESET_OPTIONS.map((o) => o.value)).not.toContain('bench_4dof');
  });

  it('bringup preset maps omit bench/arm_4dof_right profile aliases', () => {
    expect(PRESET_TO_PROFILE).not.toHaveProperty('bench_4dof');
    expect(PROFILE_TO_PRESET).not.toHaveProperty('arm_4dof_right');
    expect(JSON.stringify(PRESET_TO_PROFILE)).not.toContain('bench_4dof');
    expect(JSON.stringify(PROFILE_TO_PRESET)).not.toContain('arm_4dof_right');
  });
});
