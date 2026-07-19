// @vitest-environment node
import { afterEach, describe, expect, it, vi } from 'vitest';

import { getSidebarNavMain } from '@/data/sidebar-nav';

describe('getSidebarNavMain actuators gating', () => {
  afterEach(() => {
    vi.unstubAllEnvs();
  });

  it('omits Actuators when VITE_FEATURE_ACTUATORS is false', () => {
    vi.stubEnv('VITE_FEATURE_ACTUATORS', 'false');
    const titles = getSidebarNavMain().map((item) => item.title);
    expect(titles).not.toContain('Actuators');
  });

  it('includes Actuators linking to /actuators when flag is true', () => {
    vi.stubEnv('VITE_FEATURE_ACTUATORS', 'true');
    const actuators = getSidebarNavMain().find((item) => item.title === 'Actuators');
    expect(actuators).toBeDefined();
    expect(actuators?.url).toBe('/actuators');
    expect(actuators?.icon).toBe('actuators');
  });

  it('omits stub destinations from main nav', () => {
    vi.stubEnv('VITE_FEATURE_ACTUATORS', 'false');
    const urls = getSidebarNavMain().map((item) => item.url);
    expect(urls.every((url) => url.startsWith('/'))).toBe(true);
    expect(urls).not.toContain('#');
    expect(getSidebarNavMain().map((item) => item.title)).not.toContain('Visualizer');
    expect(getSidebarNavMain().map((item) => item.title)).not.toContain('Safety');
  });
});