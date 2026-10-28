import { describe, expect, it } from 'vitest';

import { JointHomingState } from '@/gen/marengo/v1/marengo_pb';
import { resolveActuatorCardBadges } from '@/lib/actuator-card-badges';

describe('resolveActuatorCardBadges', () => {
  it('shows DISABLED + UNHOMED when idle and not zeroed', () => {
    const badges = resolveActuatorCardBadges({
      operationalMode: 'DISABLED',
      driveActive: false,
      homingState: JointHomingState.UNHOMED,
      fault: 0,
    });
    expect(badges.map((b) => b.label)).toEqual(['DISABLED', 'UNHOMED']);
  });

  it('shows ENABLED + ZEROED for a driven, verified joint', () => {
    const badges = resolveActuatorCardBadges({
      operationalMode: 'ACTIVE',
      driveActive: true,
      homingState: JointHomingState.VERIFIED,
      fault: 0,
    });
    expect(badges.map((b) => b.label)).toEqual(['ENABLED', 'ZEROED']);
  });

  it('an undriven, unverified joint reads DISABLED + UNHOMED while the robot is ACTIVE (UI-09)', () => {
    const badges = resolveActuatorCardBadges({
      operationalMode: 'ACTIVE',
      driveActive: false,
      homingState: JointHomingState.UNHOMED,
      fault: 0,
    });
    expect(badges.map((b) => b.label)).toEqual(['DISABLED', 'UNHOMED']);
  });

  it('surfaces FAULT first', () => {
    const badges = resolveActuatorCardBadges({
      operationalMode: 'ACTIVE',
      driveActive: true,
      homingState: JointHomingState.VERIFIED,
      fault: 0x10,
    });
    expect(badges[0]?.label).toMatch(/^FAULT/);
    expect(badges.map((b) => b.id)).toContain('drive');
  });
});
