import { describe, it, expect } from 'vitest';
import {
  clamp,
  interpolateKeyframes,
  waypointKeyframes,
  applyTrimAndClamp,
  computeTickCommand,
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

  describe('waypointKeyframes', () => {
    const keyframes = [
      { targetRad: 1.0, durationSec: 1.0 },
      { targetRad: 2.0, durationSec: 1.0 },
    ];

    it('holds first waypoint for the whole first segment', () => {
      const res = waypointKeyframes(keyframes, 0, 0.5, false);
      expect(res.position).toBe(1.0);
      expect(res.segmentIndex).toBe(0);
      expect(res.isFinished).toBe(false);
    });

    it('holds second waypoint in second segment', () => {
      const res = waypointKeyframes(keyframes, 0, 1.5, false);
      expect(res.position).toBe(2.0);
      expect(res.segmentIndex).toBe(1);
    });

    it('loops waypoints', () => {
      const res = waypointKeyframes(keyframes, 0, 2.5, true);
      expect(res.position).toBe(1.0);
      expect(res.segmentIndex).toBe(0);
    });

    it('finishes on last waypoint when not looping', () => {
      const res = waypointKeyframes(keyframes, 0, 2.5, false);
      expect(res.position).toBe(2.0);
      expect(res.isFinished).toBe(true);
    });
  });

  describe('interpolateKeyframes', () => {
    const keyframes = [
      { targetRad: 1.0, durationSec: 1.0 },
      { targetRad: 2.0, durationSec: 1.0 },
    ];

    it('interpolates first segment', () => {
      const res = interpolateKeyframes(keyframes, 0, 0.5, false);
      expect(res.position).toBeCloseTo(0.5);
      expect(res.isFinished).toBe(false);
    });
  });

  describe('computeTickCommand', () => {
    it('uses waypoint target with trim and clamp', () => {
      const keyframes = [{ targetRad: 1.0, durationSec: 1.0 }];

      const res = computeTickCommand({
        jointName: 'test_joint',
        keyframes,
        initialRad: 0,
        elapsedSec: 0.5,
        loop: false,
        trim: 0.5,
        minLimit: 0,
        maxLimit: 0.8,
      });

      // Waypoint 1.0 + trim 0.5 = 1.5 → clamp to 0.8
      expect(res.position).toBeCloseTo(0.8);
      expect(res.isFinished).toBe(false);
      expect(res.segmentIndex).toBe(0);
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
