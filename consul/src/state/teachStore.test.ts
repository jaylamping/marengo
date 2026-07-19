import { beforeEach, describe, expect, it, vi } from 'vitest';

import { createTeachSession, TEACH_SESSION_VERSION } from '@/lib/teach-transit';
import {
  parseTeachPersisted,
  TEACH_STORAGE_KEY,
  useTeachStore,
} from '@/state/teachStore';

const joints = [
  'right_shoulder_pitch',
  'right_shoulder_roll',
  'right_upper_arm_yaw',
  'right_elbow_pitch',
];

function session(epoch = 0) {
  return createTeachSession(
    { profile: 'arm_4dof_right', joints, deployRev: 'abc1234' },
    'wave',
    [
      {
        id: 'a',
        label: 'home',
        tSec: 0,
        q: {
          right_shoulder_pitch: 0,
          right_shoulder_roll: 0,
          right_upper_arm_yaw: 0,
          right_elbow_pitch: 0,
        },
        included: true,
      },
      {
        id: 'b',
        label: 'up',
        tSec: 2,
        q: {
          right_shoulder_pitch: 3,
          right_shoulder_roll: 0.4,
          right_upper_arm_yaw: 0.1,
          right_elbow_pitch: 1.0,
        },
        included: true,
      },
    ],
    { calibrationEpoch: epoch }
  );
}

describe('parseTeachPersisted', () => {
  it('strips legacy frozen preset and fail-closes missing epochs', () => {
    const raw = JSON.stringify({
      liveCalibrationEpoch: 3,
      overlays: {
        wave: {
          session: {
            version: TEACH_SESSION_VERSION,
            profile: 'arm_4dof_right',
            joints,
            deployRev: 'abc1234',
            presetId: 'wave',
            landmarks: [],
            cadenceScale: 1,
            settleDwellSec: 0,
            createdAtMs: 1,
            // calibrationEpoch omitted
          },
          preset: { id: 'wave', tampered: true },
          // ackedAtEpoch omitted
        },
      },
    });
    const parsed = parseTeachPersisted(raw);
    expect(parsed.liveCalibrationEpoch).toBe(3);
    expect(parsed.overlays.wave.session.calibrationEpoch).toBe(2);
    expect(parsed.overlays.wave.ackedAtEpoch).toBe(2);
    expect(
      (parsed.overlays.wave as { preset?: unknown }).preset
    ).toBeUndefined();
  });

  it('returns empty on corrupt JSON', () => {
    expect(parseTeachPersisted('{not-json')).toEqual({
      liveCalibrationEpoch: 0,
      overlays: {},
    });
  });
});

describe('useTeachStore', () => {
  beforeEach(() => {
    localStorage.clear();
    useTeachStore.setState({
      recording: false,
      gravityArmed: false,
      samples: [],
      landmarks: [],
      cadenceScale: 1,
      settleDwellSec: 0.15,
      lastError: null,
      liveCalibrationEpoch: 0,
      overlays: {},
    });
  });

  it('applyOverlay persists session without preset', () => {
    useTeachStore.getState().applyOverlay('wave', {
      session: session(0),
      ackedAtEpoch: 0,
    });
    const entry = useTeachStore.getState().overlays.wave;
    expect(entry?.session.presetId).toBe('wave');
    expect((entry as { preset?: unknown } | undefined)?.preset).toBeUndefined();
    const stored = JSON.parse(localStorage.getItem(TEACH_STORAGE_KEY)!);
    expect(stored.overlays.wave.preset).toBeUndefined();
    expect(stored.overlays.wave.session.deployRev).toBe('abc1234');
  });

  it('markCalibrationChanged bumps epoch, clears buffer, keeps overlay', () => {
    useTeachStore.getState().applyOverlay('wave', {
      session: session(0),
      ackedAtEpoch: 0,
    });
    useTeachStore.setState({
      samples: [{ tMs: 1, q: { right_shoulder_pitch: 1 } }],
      landmarks: [
        {
          id: 'x',
          label: 'x',
          tSec: 0,
          q: { right_shoulder_pitch: 0 },
          included: true,
        },
      ],
      recording: true,
    });
    useTeachStore.getState().markCalibrationChanged();
    const st = useTeachStore.getState();
    expect(st.liveCalibrationEpoch).toBe(1);
    expect(st.overlays.wave).toBeDefined();
    expect(st.samples).toEqual([]);
    expect(st.landmarks).toEqual([]);
    expect(st.recording).toBe(false);
    expect(st.lastError).toMatch(/Calibration marked dirty/);
  });

  it('acknowledgeCalibration updates ackedAtEpoch to live', () => {
    useTeachStore.getState().applyOverlay('wave', {
      session: session(0),
      ackedAtEpoch: 0,
    });
    useTeachStore.setState({ liveCalibrationEpoch: 2 });
    useTeachStore.getState().acknowledgeCalibration('wave');
    expect(useTeachStore.getState().overlays.wave.ackedAtEpoch).toBe(2);
  });

  it('persist failure surfaces lastError and does not bump epoch', () => {
    useTeachStore.getState().applyOverlay('wave', {
      session: session(0),
      ackedAtEpoch: 0,
    });
    const spy = vi
      .spyOn(Storage.prototype, 'setItem')
      .mockImplementation(() => {
        throw new Error('quota');
      });
    useTeachStore.getState().markCalibrationChanged();
    expect(useTeachStore.getState().liveCalibrationEpoch).toBe(0);
    expect(useTeachStore.getState().lastError).toMatch(/localStorage/);
    spy.mockRestore();
  });

  it('appendSample no-ops when not recording', () => {
    useTeachStore.getState().appendSample({
      tMs: 1,
      q: { right_shoulder_pitch: 0.5 },
    });
    expect(useTeachStore.getState().samples).toEqual([]);
  });
});
