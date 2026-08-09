import { describe, expect, it } from 'vitest';

import { scopeWidens } from '@/lib/commissioning-scope';

describe('scopeWidens', () => {
  it('requires confirm when effective set grows', () => {
    expect(scopeWidens(['roll', 'pitch'], ['roll', 'pitch', 'yaw'])).toBe(true);
  });

  it('does not widen on narrow or equal', () => {
    expect(scopeWidens(['roll', 'pitch', 'yaw'], ['roll', 'pitch'])).toBe(false);
    expect(scopeWidens(['roll'], ['roll'])).toBe(false);
  });
});
