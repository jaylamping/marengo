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
  });

  it('shows ENABLED + ZEROED when ACTIVE', () => {
    const badges = resolveActuatorCardBadges({
      operationalMode: 'ACTIVE',
      zeroed: false,
      fault: 0,
    });
    expect(badges.map((b) => b.label)).toEqual(['ENABLED', 'ZEROED']);
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
