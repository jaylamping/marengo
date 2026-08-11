import { describe, expect, it } from 'vitest';

import { sidebarContextLabel } from '@/data/sidebar-nav';

describe('sidebarContextLabel', () => {
  it('is local outside production builds', () => {
    expect(sidebarContextLabel(false)).toBe('local');
  });

  it('is live for production builds', () => {
    expect(sidebarContextLabel(true)).toBe('live');
  });
});
