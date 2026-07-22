import { describe, expect, it } from 'vitest';

import { resolveActuatorCardBadges } from '@/lib/actuator-card-badges';

describe('resolveActuatorCardBadges', () => {
  it('shows DISABLED + UNHOMED when idle and not zeroed', () => {
    const badges = resolveActuatorCardBadges({
      operationalMode: 'DISABLED',
      zeroed: false,
      fault: 0,
    });
    expect(badges.map((b) => b.label)).toEqual(['DISABLED', 'UNHOMED']);
    const zero = badges.find((b) => b.id === 'zero');
    expect(zero?.presentation).toBe('icon');
    expect(zero?.tone).toBe('warning');
    expect(zero?.detail).toMatch(/Set zero/i);
  });

  it('shows ENABLED + ZEROED when ACTIVE', () => {
    const badges = resolveActuatorCardBadges({
      operationalMode: 'ACTIVE',
      zeroed: false,
      fault: 0,
    });
    expect(badges.map((b) => b.label)).toEqual(['ENABLED', 'ZEROED']);
    const zero = badges.find((b) => b.id === 'zero');
    expect(zero?.presentation).toBe('icon');
    expect(zero?.tone).toBe('ok');
    expect(zero?.detail).toMatch(/origin confirmed/i);
  });

  it('surfaces FAULT first', () => {
    const badges = resolveActuatorCardBadges({
      operationalMode: 'ACTIVE',
      zeroed: true,
      fault: 0x10,
    });
    expect(badges[0]?.label).toMatch(/^FAULT/);
    expect(badges.map((b) => b.id)).toContain('drive');
  });
});
