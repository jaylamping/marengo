import { describe, expect, it } from 'vitest';

import {
  canApplyLandmarks,
  extractLandmarks,
  proposeExtremaTimes,
  samplesHaveMotion,
  snapshotAtTime,
  type TeachSample,
} from '@/lib/teach-record';

const JOINTS = [
  'right_shoulder_pitch',
  'right_shoulder_roll',
  'right_upper_arm_yaw',
  'right_elbow_pitch',
] as const;

function makeWaveSamples(): TeachSample[] {
  const samples: TeachSample[] = [];
  const t0 = 1_000_000;
  // Home dwell
  for (let i = 0; i < 20; i++) {
    samples.push({
      tMs: t0 + i * 50,
      q: {
        right_shoulder_pitch: 0.01,
        right_shoulder_roll: 0,
        right_upper_arm_yaw: 0,
        right_elbow_pitch: 0,
      },
    });
  }
  // Raise
  for (let i = 0; i < 40; i++) {
    const u = i / 39;
    samples.push({
      tMs: t0 + 1000 + i * 50,
      q: {
        right_shoulder_pitch: 3.0 * u,
        right_shoulder_roll: 0.42 * u,
        right_upper_arm_yaw: 0.1 * Math.sin(u * Math.PI),
        right_elbow_pitch: 1.0 * u,
      },
    });
  }
  // Roll wave + yaw wiggle
  for (let i = 0; i < 80; i++) {
    const phase = (i / 80) * Math.PI * 2;
    samples.push({
      tMs: t0 + 3000 + i * 50,
      q: {
        right_shoulder_pitch: 3.0,
        right_shoulder_roll: 0.56 + 0.14 * Math.sin(phase),
        right_upper_arm_yaw: 0.2 * Math.sin(phase * 0.5),
        right_elbow_pitch: 1.0,
      },
    });
  }
  return samples;
}

describe('teach-record landmarks', () => {
  it('snapshots full-q at a time (never invents missing joints)', () => {
    const samples = makeWaveSamples();
    const q = snapshotAtTime(samples, [...JOINTS], 3.5);
    expect(q).not.toBeNull();
    expect(q!.right_shoulder_pitch).toBeCloseTo(3.0, 1);
    expect(Object.keys(q!).sort()).toEqual([...JOINTS].sort());
  });

  it('proposes extrema times without stitching poses', () => {
    const samples = makeWaveSamples();
    const times = proposeExtremaTimes(samples, 'right_shoulder_roll', {
      minProminence: 0.05,
    });
    expect(times.length).toBeGreaterThan(0);
  });

  it('extracts landmarks with full-q snapshots', () => {
    const landmarks = extractLandmarks(makeWaveSamples(), [...JOINTS]);
    expect(landmarks.length).toBeGreaterThanOrEqual(2);
    for (const lm of landmarks) {
      for (const j of JOINTS) {
        expect(typeof lm.q[j]).toBe('number');
      }
    }
    expect(canApplyLandmarks(landmarks)).toBe(true);
  });

  it('fail-closed on empty / noise-only buffer', () => {
    const noise: TeachSample[] = Array.from({ length: 10 }, (_, i) => ({
      tMs: i * 100,
      q: {
        right_shoulder_pitch: 0.001 * (i % 2),
        right_shoulder_roll: 0,
        right_upper_arm_yaw: 0,
        right_elbow_pitch: 0,
      },
    }));
    expect(samplesHaveMotion(noise, [...JOINTS])).toBe(false);
    expect(extractLandmarks(noise, [...JOINTS])).toEqual([]);
    expect(canApplyLandmarks([])).toBe(false);
  });

  it('fail-closed when dropout omits a joint at snapshot time', () => {
    const samples = makeWaveSamples();
    const t0 = samples[0].tMs;
    const dropIdx = 50;
    const dropTSec = (samples[dropIdx].tMs - t0) / 1000;
    samples[dropIdx] = {
      ...samples[dropIdx],
      q: { right_shoulder_pitch: 1, right_shoulder_roll: 0.2 },
    };
    expect(snapshotAtTime(samples, [...JOINTS], dropTSec)).toBeNull();
  });
});
