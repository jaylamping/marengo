import { describe, it, expect } from 'vitest';
import { clamp, interpolateKeyframes, applyTrimAndClamp, computeTickCommand } from './compound-runner';

describe('compound-runner', () => {
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

    it('interpolates second segment', () => {
      const res = interpolateKeyframes(keyframes, 0, 1.5, false);
      expect(res.position).toBeCloseTo(1.5);
      expect(res.isFinished).toBe(false);
    });

    it('handles loop', () => {
      const res = interpolateKeyframes(keyframes, 0, 2.5, true);
      expect(res.position).toBeCloseTo(1.5);
      expect(res.isFinished).toBe(false);
    });
    
    it('finishes when not looping', () => {
      const res = interpolateKeyframes(keyframes, 0, 2.5, false);
      expect(res.position).toBe(2.0);
      expect(res.isFinished).toBe(true);
    });
  });

  describe('computeTickCommand', () => {
    it('computes final position with trim and clamp', () => {
      const keyframes = [
        { targetRad: 1.0, durationSec: 1.0 },
      ];
      
      const res = computeTickCommand({
        jointName: 'test_joint',
        keyframes,
        initialRad: 0,
        elapsedSec: 0.5,
        loop: false,
        trim: 0.5,
        minLimit: 0,
        maxLimit: 0.8, // Clamp should kick in
      });
      
      // Interpolated position at 0.5s is 0.5
      // Trim is 0.5 -> 1.0
      // Clamp max is 0.8 -> 0.8
      expect(res.position).toBeCloseTo(0.8);
      expect(res.isFinished).toBe(false);
    });
  });
});
