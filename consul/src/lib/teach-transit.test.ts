import { describe, expect, it } from 'vitest';

import { COMPOUND_TEST_PRESETS } from '@/data/compound-tests';
import type { TeachLandmark } from '@/lib/teach-record';
import {
  createTeachSession,
  fingerprintsMatch,
  liveFingerprint,
  landmarksToKeyframes,
  overlayReplayAllowed,
  resolvePlayablePreset,
  sessionToWaveOverlay,
  TEACH_SESSION_VERSION,
} from '@/lib/teach-transit';

const joints = [
  'right_shoulder_pitch',
  'right_shoulder_roll',
  'right_upper_arm_yaw',
];

function landmarks(): TeachLandmark[] {
  return [
    {
      id: 'a',
      label: 'home',
      tSec: 0,
      q: {
        right_shoulder_pitch: 0,
        right_shoulder_roll: 0,
        right_upper_arm_yaw: 0,
      },
      included: true,
    },
    {
      id: 'b',
      label: 'upright',
      tSec: 2,
      q: {
        right_shoulder_pitch: 3,
        right_shoulder_roll: 0.42,
        right_upper_arm_yaw: 0.1,
      },
      included: true,
    },
    {
      id: 'c',
      label: 'roll_extrema',
      tSec: 3.4,
      q: {
        right_shoulder_pitch: 3,
        right_shoulder_roll: 0.7,
        right_upper_arm_yaw: 0.05,
      },
      included: true,
    },
  ];
}

describe('teach-transit cadence/dwell', () => {
  it('scales taught Δt without inventing berthier speed', () => {
    const kf1 = landmarksToKeyframes(landmarks(), joints, 1, 0)!;
    const kf2 = landmarksToKeyframes(landmarks(), joints, 2, 0)!;
    // Second segment duration scales with cadence (first is approach floor).
    expect(kf2.right_shoulder_roll[1].durationSec).toBeCloseTo(
      kf1.right_shoulder_roll[1].durationSec * 2,
      5
    );
  });

  it('adds settle dwell once (not double-multiplied by runner)', () => {
    const noDwell = landmarksToKeyframes(landmarks(), joints, 1, 0)!;
    const withDwell = landmarksToKeyframes(landmarks(), joints, 1, 0.25)!;
    expect(withDwell.right_shoulder_roll[1].durationSec).toBeCloseTo(
      noDwell.right_shoulder_roll[1].durationSec + 0.25,
      5
    );
  });

  it('fail-closed on empty landmarks', () => {
    expect(landmarksToKeyframes([], joints, 1, 0)).toBeNull();
    const wave = COMPOUND_TEST_PRESETS.find((p) => p.id === 'wave')!;
    const session = createTeachSession(
      { profile: 'arm_3dof_right', joints, deployRev: 'abc1234' },
      'wave',
      []
    );
    const result = sessionToWaveOverlay(session, wave, session);
    expect(result.ok).toBe(false);
    if (!result.ok) expect(result.error).toBe('empty_landmarks');
  });

  it('refuses Apply on fingerprint mismatch', () => {
    const wave = COMPOUND_TEST_PRESETS.find((p) => p.id === 'wave')!;
    const session = createTeachSession(
      { profile: 'arm_3dof_right', joints, deployRev: 'aaa1111' },
      'wave',
      landmarks()
    );
    const result = sessionToWaveOverlay(session, wave, {
      profile: 'arm_3dof_right',
      joints,
      deployRev: 'bbb2222',
    });
    expect(result.ok).toBe(false);
    if (!result.ok) expect(result.error).toBe('fingerprint_mismatch');
  });

  it('builds overlay without nativeWave and with loopFromSegment', () => {
    const wave = COMPOUND_TEST_PRESETS.find((p) => p.id === 'wave')!;
    expect(wave.nativeWave).toBeDefined();
    const fp = { profile: 'arm_3dof_right', joints, deployRev: 'abc1234' };
    const session = createTeachSession(fp, 'wave', landmarks());
    const result = sessionToWaveOverlay(session, wave, fp);
    expect(result.ok).toBe(true);
    if (!result.ok) return;
    expect(result.preset.nativeWave).toBeUndefined();
    expect(result.preset.loopFromSegment).toBe(1);
    expect(result.preset.loop).toBe(true);
  });

  it('two-landmark overlay does not loop (avoids re-raise)', () => {
    const wave = COMPOUND_TEST_PRESETS.find((p) => p.id === 'wave')!;
    const fp = { profile: 'arm_3dof_right', joints, deployRev: 'abc1234' };
    const session = createTeachSession(fp, 'wave', landmarks().slice(0, 2));
    const result = sessionToWaveOverlay(session, wave, fp);
    expect(result.ok).toBe(true);
    if (!result.ok) return;
    expect(result.preset.loop).toBe(false);
    expect(result.preset.loopFromSegment).toBeUndefined();
  });

  it('sessionToWaveOverlay refuses version_mismatch and preset_mismatch', () => {
    const wave = COMPOUND_TEST_PRESETS.find((p) => p.id === 'wave')!;
    const fp = { profile: 'arm_3dof_right', joints, deployRev: 'abc1234' };
    const session = createTeachSession(fp, 'wave', landmarks());
    const badVersion = sessionToWaveOverlay(
      { ...session, version: 99 as typeof TEACH_SESSION_VERSION },
      wave,
      fp
    );
    expect(badVersion.ok).toBe(false);
    if (!badVersion.ok) expect(badVersion.error).toBe('version_mismatch');
    const badPreset = sessionToWaveOverlay(
      { ...session, presetId: 'not-wave' },
      wave,
      fp
    );
    expect(badPreset.ok).toBe(false);
    if (!badPreset.ok) expect(badPreset.error).toBe('preset_mismatch');
  });

  it('landmarksToKeyframes fails closed when a joint is missing from q', () => {
    const partial = landmarks().map((lm, i) =>
      i === 0
        ? lm
        : {
            ...lm,
            q: {
              right_shoulder_pitch: lm.q.right_shoulder_pitch,
              right_shoulder_roll: lm.q.right_shoulder_roll,
              // yaw omitted — inventing 0 would be unsafe
            },
          }
    );
    expect(landmarksToKeyframes(partial, joints, 1, 0)).toBeNull();
    const wave = COMPOUND_TEST_PRESETS.find((p) => p.id === 'wave')!;
    const fp = { profile: 'arm_3dof_right', joints, deployRev: 'abc1234' };
    const session = createTeachSession(fp, 'wave', partial);
    const result = sessionToWaveOverlay(session, wave, fp);
    expect(result.ok).toBe(false);
    if (!result.ok) expect(result.error).toBe('empty_landmarks');
  });


  it('liveFingerprint falls back from empty deployRev to gitSha', () => {
    const fp = liveFingerprint('arm_3dof_right', joints, {
      deployRev: '',
      gitSha: 'abcdef1234567',
    });
    expect(fp.deployRev).toBe('abcdef1234567');
    expect(
      fingerprintsMatch(
        fp,
        liveFingerprint('arm_3dof_right', joints, { gitSha: 'abcdef1234567' })
      )
    ).toBe(true);
  });

  it('overlayReplayAllowed checks version + fingerprint', () => {
    const fp = { profile: 'arm_3dof_right', joints, deployRev: 'abc1234' };
    const session = createTeachSession(fp, 'wave', landmarks(), {
      calibrationEpoch: 0,
    });
    expect(overlayReplayAllowed(session, fp).ok).toBe(true);
    expect(
      overlayReplayAllowed(
        { ...session, version: 99 as typeof TEACH_SESSION_VERSION },
        fp
      ).ok
    ).toBe(false);
    expect(fingerprintsMatch(fp, { ...fp, deployRev: 'unknown' })).toBe(false);
    expect(fingerprintsMatch({ ...fp, deployRev: 'unknown' }, fp)).toBe(false);
    expect(
      overlayReplayAllowed(session, { ...fp, deployRev: 'different' }).ok
    ).toBe(false);
  });

  it('resolvePlayablePreset falls back to shipped Wave when overlay refused', () => {
    const wave = COMPOUND_TEST_PRESETS.find((p) => p.id === 'wave')!;
    const fp = { profile: 'arm_3dof_right', joints, deployRev: 'abc1234' };
    const session = createTeachSession(fp, 'wave', landmarks(), {
      calibrationEpoch: 0,
    });
    const ok = resolvePlayablePreset(
      wave,
      { session, ackedAtEpoch: 0 },
      fp,
      0
    );
    expect(ok.usingOverlay).toBe(true);
    expect(ok.preset.nativeWave).toBeUndefined();

    const dirty = resolvePlayablePreset(
      wave,
      { session, ackedAtEpoch: 0 },
      fp,
      1
    );
    expect(dirty.usingOverlay).toBe(false);
    expect(dirty.preset).toBe(wave);
    expect(dirty.warning).toMatch(/Acknowledge/);

    const mismatch = resolvePlayablePreset(
      wave,
      { session, ackedAtEpoch: 0 },
      { ...fp, deployRev: 'otherrev' },
      0
    );
    expect(mismatch.usingOverlay).toBe(false);
    expect(mismatch.preset.nativeWave).toEqual(wave.nativeWave);
  });
});
