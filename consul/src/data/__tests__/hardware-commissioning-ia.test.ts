// @vitest-environment node
import { describe, expect, it } from 'vitest';

import { getSidebarNavMain } from '@/data/sidebar-nav';
import { siteHeaderConfig } from '@/data/site-header';
import {
  PRESET_TO_PROFILE,
  PROFILE_TO_PRESET,
} from '@/lib/bringup-presets';
import { appRoutes } from '@/routes/config';

function childRoutes() {
  const root = appRoutes[0];
  if (!root?.children) {
    throw new Error('expected RootLayout children');
  }
  return root.children;
}

describe('hardware commissioning IA — sidebar', () => {
  it('targets Telemetry at /telemetry with the telemetry icon', () => {
    const telemetry = getSidebarNavMain().find((item) => item.title === 'Telemetry');
    expect(telemetry).toEqual({
      title: 'Telemetry',
      url: '/telemetry',
      icon: 'telemetry',
    });
  });

  it('omits Actuators and Subsystems from primary nav', () => {
    const titles = getSidebarNavMain().map((item) => item.title);
    expect(titles).not.toContain('Actuators');
    expect(titles).not.toContain('Subsystems');
    const urls = getSidebarNavMain().map((item) => item.url);
    expect(urls).not.toContain('/actuators');
    expect(urls).not.toContain('/subsystems');
  });
});

describe('hardware commissioning IA — routes', () => {
  it('registers /telemetry with Telemetry chrome', () => {
    const telemetry = childRoutes().find((route) => route.path === '/telemetry');
    expect(telemetry).toBeDefined();
    const handle = telemetry?.handle as { header?: { title?: string; subtitle?: string } };
    expect(handle.header?.title).toBe('Telemetry');
    expect(handle.header?.subtitle).toMatch(/read-only|live/i);
  });

  it('does not register /actuators', () => {
    expect(childRoutes().some((route) => route.path === '/actuators')).toBe(false);
  });

  it('keeps /subsystems only as a redirect shell path', () => {
    const subsystems = childRoutes().find((route) => route.path === '/subsystems');
    expect(subsystems).toBeDefined();
    expect(subsystems?.Component).toBeDefined();
  });
});

describe('hardware commissioning IA — master chrome', () => {
  it('uses master inventory language on overview route subtitle', () => {
    const overview = childRoutes().find((route) => route.path === '/');
    const handle = overview?.handle as { header?: { subtitle?: string } };
    const subtitle = handle.header?.subtitle ?? '';
    expect(subtitle).toMatch(/master/i);
    expect(subtitle).not.toMatch(/arm_4dof_right|bench_4dof|bench\b/i);
  });

  it('uses master inventory language in site header fallback', () => {
    expect(siteHeaderConfig.subtitle).toMatch(/master/i);
    expect(siteHeaderConfig.subtitle).not.toMatch(/arm_4dof|bench_4dof|marengo_arm_4dof/i);
  });

  it('drops arm_4dof_right / bench_4dof from bringup preset maps', () => {
    expect(PRESET_TO_PROFILE).not.toHaveProperty('bench_4dof');
    expect(PROFILE_TO_PRESET).not.toHaveProperty('arm_4dof_right');
    expect(JSON.stringify(PRESET_TO_PROFILE)).not.toContain('bench_4dof');
    expect(JSON.stringify(PROFILE_TO_PRESET)).not.toContain('arm_4dof_right');
  });
});
