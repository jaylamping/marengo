// @vitest-environment node
import { afterEach, describe, expect, it, vi } from 'vitest';

import { isActuatorsFeatureEnabled } from '@/lib/feature-flags';

describe('isActuatorsFeatureEnabled', () => {
  afterEach(() => {
    vi.unstubAllEnvs();
  });

  it('returns false when VITE_FEATURE_ACTUATORS is unset', () => {
    vi.stubEnv('VITE_FEATURE_ACTUATORS', '');
    expect(isActuatorsFeatureEnabled()).toBe(false);
  });

  it('returns false when VITE_FEATURE_ACTUATORS is not "true"', () => {
    vi.stubEnv('VITE_FEATURE_ACTUATORS', 'false');
    expect(isActuatorsFeatureEnabled()).toBe(false);
  });

  it('returns true only when VITE_FEATURE_ACTUATORS is exactly "true"', () => {
    vi.stubEnv('VITE_FEATURE_ACTUATORS', 'true');
    expect(isActuatorsFeatureEnabled()).toBe(true);
  });
});
