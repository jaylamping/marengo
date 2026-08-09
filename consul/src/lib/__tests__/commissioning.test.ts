import { describe, expect, it } from 'vitest';

import {
  MASTER_LIMBS,
  aggregateWorstBadge,
  isReferenceReady,
  limbReady,
  resolveJointBadge,
  robotReady,
  wireFacetsLive,
  type CommissioningBadge,
  type JointFacetSnapshot,
} from '@/lib/commissioning';
import { JointHomingState } from '@/gen/marengo/v1/marengo_pb';

function facet(partial: Partial<JointFacetSnapshot> & { name: string }): JointFacetSnapshot {
  return {
    online: true,
    motorMapped: true,
    fault: 0,
    outOfLimits: false,
    driveActive: false,
    homingState: JointHomingState.VERIFIED,
    ...partial,
  };
}

describe('wireFacetsLive', () => {
  it('is false when homing_state is absent or UNSPECIFIED', () => {
    expect(wireFacetsLive(undefined)).toBe(false);
    expect(wireFacetsLive(JointHomingState.UNSPECIFIED)).toBe(false);
    expect(wireFacetsLive('UNSPECIFIED')).toBe(false);
  });

  it('is true for non-UNSPECIFIED wire values', () => {
    expect(wireFacetsLive(JointHomingState.VERIFIED)).toBe(true);
    expect(wireFacetsLive(JointHomingState.UNHOMED)).toBe(true);
    expect(wireFacetsLive('FAULTED')).toBe(true);
  });
});

describe('isReferenceReady (wire only)', () => {
  it('is true only for Verified wire — never localStorage hints', () => {
    expect(isReferenceReady(JointHomingState.VERIFIED)).toBe(true);
    expect(isReferenceReady('VERIFIED')).toBe(true);
    expect(isReferenceReady(JointHomingState.UNHOMED)).toBe(false);
    expect(isReferenceReady(undefined)).toBe(false);
    expect(isReferenceReady(JointHomingState.UNSPECIFIED)).toBe(false);
  });
});

describe('resolveJointBadge priority Fault>OutOfLimits>Offline>Active>Ready>Online', () => {
  it('Fault beats Active', () => {
    expect(
      resolveJointBadge(
        facet({
          name: 'right_shoulder_pitch',
          driveActive: true,
          fault: 0x10,
          homingState: JointHomingState.VERIFIED,
        }),
      ),
    ).toBe('Fault');
  });

  it('OutOfLimits beats Ready', () => {
    expect(
      resolveJointBadge(
        facet({
          name: 'right_shoulder_pitch',
          outOfLimits: true,
          homingState: JointHomingState.VERIFIED,
        }),
      ),
    ).toBe('OutOfLimits');
  });

  it('Fault beats OutOfLimits', () => {
    expect(
      resolveJointBadge(
        facet({
          name: 'j',
          fault: 1,
          outOfLimits: true,
          homingState: JointHomingState.FAULTED,
        }),
      ),
    ).toBe('Fault');
  });

  it('Offline beats Active/Ready/Online when no feedback', () => {
    expect(
      resolveJointBadge(
        facet({
          name: 'j',
          online: false,
          motorMapped: true,
          driveActive: true,
          homingState: JointHomingState.VERIFIED,
        }),
      ),
    ).toBe('Offline');
  });

  it('Active beats Ready', () => {
    expect(
      resolveJointBadge(
        facet({
          name: 'j',
          driveActive: true,
          homingState: JointHomingState.VERIFIED,
        }),
      ),
    ).toBe('Active');
  });

  it('Ready beats Online when Verified and idle', () => {
    expect(
      resolveJointBadge(
        facet({
          name: 'j',
          driveActive: false,
          homingState: JointHomingState.VERIFIED,
        }),
      ),
    ).toBe('Ready');
  });

  it('Online when live but not Verified', () => {
    expect(
      resolveJointBadge(
        facet({
          name: 'j',
          homingState: JointHomingState.UNHOMED,
        }),
      ),
    ).toBe('Online');
  });

  it('Unknown when wire facets are not live', () => {
    expect(
      resolveJointBadge(
        facet({
          name: 'j',
          homingState: JointHomingState.UNSPECIFIED,
        }),
      ),
    ).toBe('Unknown');
  });

  it('treats FAULTED homing without OOL as Fault', () => {
    expect(
      resolveJointBadge(
        facet({
          name: 'j',
          fault: 0,
          outOfLimits: false,
          homingState: JointHomingState.FAULTED,
        }),
      ),
    ).toBe('Fault');
  });
});

describe('aggregateWorstBadge', () => {
  it('picks highest priority across members', () => {
    const badges: CommissioningBadge[] = ['Ready', 'Online', 'OutOfLimits', 'Active'];
    expect(aggregateWorstBadge(badges)).toBe('OutOfLimits');
  });

  it('Fault wins over everything', () => {
    expect(aggregateWorstBadge(['Online', 'Fault', 'Ready'])).toBe('Fault');
  });
});

describe('limbReady / robotReady aggregation', () => {
  it('unbuilt Offline members do not block Limb or Robot Ready', () => {
    const built = [
      facet({ name: 'right_shoulder_roll' }),
      facet({ name: 'right_shoulder_pitch' }),
      facet({ name: 'right_upper_arm_yaw' }),
      facet({ name: 'right_elbow_pitch' }),
    ];
    const unbuilt = facet({
      name: 'right_lower_arm_yaw',
      online: false,
      motorMapped: false,
      homingState: JointHomingState.UNHOMED,
    });
    expect(limbReady([...built, unbuilt])).toBe(true);
    expect(robotReady([...built, unbuilt])).toBe(true);
  });

  it('scope-sized Ready set does not fabricate Robot Ready when another built joint is Unhomed', () => {
    const master = [
      facet({ name: 'right_shoulder_roll' }),
      facet({ name: 'right_shoulder_pitch' }),
      facet({
        name: 'right_upper_arm_yaw',
        homingState: JointHomingState.UNHOMED,
      }),
      facet({ name: 'right_elbow_pitch' }),
    ];
    expect(robotReady(master)).toBe(false);
  });

  it('OutOfLimits or Fault blocks Ready aggregation even when Verified', () => {
    expect(
      robotReady([
        facet({ name: 'a', outOfLimits: true }),
      ]),
    ).toBe(false);
    expect(
      limbReady([
        facet({ name: 'a', fault: 1, homingState: JointHomingState.FAULTED }),
      ]),
    ).toBe(false);
  });

  it('exposes anatomical MASTER_LIMBS matching robot.yaml right_arm', () => {
    expect(MASTER_LIMBS.right_arm).toEqual([
      'right_shoulder_roll',
      'right_shoulder_pitch',
      'right_upper_arm_yaw',
      'right_elbow_pitch',
      'right_lower_arm_yaw',
    ]);
  });
});
