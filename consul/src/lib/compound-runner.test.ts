import { describe, it, expect } from 'vitest';
import {
  clamp,
  applyTrimAndClamp,
  jointsSettled,
  segmentTargets,
  encodeNativeWaveCommandName,
  nativeWaveDurationSec,
  WAYPOINT_SETTLE_RAD,
} from './compound-runner';

describe('compound-runner', () => {
  describe('nativeWave helpers', () => {
    it('encodes wave command name and duration', () => {
      const wave = {
        joint: 'right_shoulder_roll',
        minRad: 0.42,
        maxRad: 0.7,
        cycles: 8,
        halfPeriodSec: 0.75,
      };
      expect(encodeNativeWaveCommandName(wave)).toBe(
        'wave:right_shoulder_roll:0.42:0.7:8:0.75'
      );
      expect(nativeWaveDurationSec(wave)).toBeCloseTo(12);
      expect(
        nativeWaveDurationSec({
          joint: 'right_shoulder_roll',
          minRad: 0.42,
          maxRad: 0.7,
          cycles: 6,
          halfPeriodSec: 1.0,
        })
      ).toBeCloseTo(12);
    });
  });

  describe('clamp', () => {
    it('clamps values correctly', () => {
      expect(clamp(5, 0, 10)).toBe(5);
      expect(clamp(-5, 0, 10)).toBe(0);
      expect(clamp(15, 0, 10)).toBe(10);
    });
  });

  describe('applyTrimAndClamp', () => {
    it('applies trim and clamps', () => {
      expect(applyTrimAndClamp(1.0, 0.5, 0, 2.0)).toBe(1.5);
      expect(applyTrimAndClamp(1.0, 1.5, 0, 2.0)).toBe(2.0);
      expect(applyTrimAndClamp(1.0, -1.5, 0, 2.0)).toBe(0.0);
    });
  });

  describe('jointsSettled', () => {
    it('requires all joints within settle radius', () => {
      expect(
        jointsSettled(
          { a: 1.0, b: 2.0 },
          { a: 1.04, b: 2.02 },
          WAYPOINT_SETTLE_RAD
        )
      ).toBe(true);
      expect(jointsSettled({ a: 1.0 }, { a: 1.0 + WAYPOINT_SETTLE_RAD + 0.01 })).toBe(false);
      expect(jointsSettled({ a: undefined }, { a: 1.0 })).toBe(false);
    });

    it('also requires low velocity when vel map provided', () => {
      expect(
        jointsSettled({ a: 1.0 }, { a: 1.02 }, WAYPOINT_SETTLE_RAD, { a: 0.05 })
      ).toBe(true);
      expect(
        jointsSettled({ a: 1.0 }, { a: 1.02 }, WAYPOINT_SETTLE_RAD, { a: 0.5 })
      ).toBe(false);
    });
  });

  describe('segmentTargets', () => {
    it('applies trim and clamp per joint', () => {
      const targets = segmentTargets(
        ['pitch', 'roll'],
        {
          pitch: [{ targetRad: 3.05, durationSec: 4 }],
          roll: [{ targetRad: 0.35, durationSec: 4 }],
        },
        0,
        { pitch: 0, roll: 0.1 },
        {
          pitch: { min: 0, max: 3.05 },
          roll: { min: 0, max: Math.PI },
        }
      );
      expect(targets.pitch).toBeCloseTo(3.05);
      expect(targets.roll).toBeCloseTo(0.45);
    });
  });
});
