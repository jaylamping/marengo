import { describe, expect, it } from 'vitest';

import { COMPOUND_TEST_PRESETS } from '@/data/compound-tests';

describe('compound presets', () => {
  // Berthier refuses a PositionWave more than 0.02 rad from the joint's held
  // target, so the raise must leave the wave joint at the wave's start.
  it('ends every native wave raise at the wave start', () => {
    for (const preset of COMPOUND_TEST_PRESETS) {
      const wave = preset.nativeWave;
      if (!wave) continue;
      const frames = preset.keyframes[wave.joint] ?? [];
      const last = frames.at(-1);
      expect(last, `${preset.id} raises ${wave.joint}`).toBeDefined();
      expect(Math.abs((last?.targetRad ?? NaN) - wave.minRad)).toBeLessThanOrEqual(0.02);
    }
  });
});
