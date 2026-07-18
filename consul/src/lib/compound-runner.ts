import { Keyframe } from '@/data/compound-tests';

export function clamp(value: number, min: number, max: number): number {
  return Math.max(min, Math.min(max, value));
}

export function interpolateKeyframes(
  keyframes: Keyframe[],
  initialRad: number,
  elapsedSec: number,
  loop: boolean
): { position: number; isFinished: boolean } {
  if (keyframes.length === 0) return { position: initialRad, isFinished: true };

  const totalDuration = keyframes.reduce((sum, kf) => sum + kf.durationSec, 0);
  
  if (totalDuration === 0) {
    return { position: keyframes[keyframes.length - 1].targetRad, isFinished: true };
  }

  let t = elapsedSec;
  let isFinished = false;
  let loopCount = 0;
  
  if (loop) {
    loopCount = Math.floor(t / totalDuration);
    t = t % totalDuration;
  } else if (t >= totalDuration) {
    t = totalDuration;
    isFinished = true;
    return { position: keyframes[keyframes.length - 1].targetRad, isFinished };
  }

  let accumulatedTime = 0;
  // If we've looped at least once, the start of the first segment is the end of the last segment.
  let prevRad = loopCount > 0 ? keyframes[keyframes.length - 1].targetRad : initialRad;

  for (let i = 0; i < keyframes.length; i++) {
    const kf = keyframes[i];
    const segmentStart = accumulatedTime;
    const segmentEnd = accumulatedTime + kf.durationSec;

    if (t >= segmentStart && t <= segmentEnd) {
      const segmentProgress = kf.durationSec > 0 ? (t - segmentStart) / kf.durationSec : 1;
      // Smooth easing (cosine)
      const ease = (1 - Math.cos(segmentProgress * Math.PI)) / 2;
      const position = prevRad + (kf.targetRad - prevRad) * ease;
      return { position, isFinished };
    }

    prevRad = kf.targetRad;
    accumulatedTime = segmentEnd;
  }

  return { position: keyframes[keyframes.length - 1].targetRad, isFinished: true };
}

export function applyTrimAndClamp(
  position: number,
  trim: number,
  minLimit: number,
  maxLimit: number
): number {
  return clamp(position + trim, minLimit, maxLimit);
}

export interface TickInput {
  jointName: string;
  keyframes: Keyframe[];
  initialRad: number;
  elapsedSec: number;
  loop: boolean;
  trim: number;
  minLimit: number;
  maxLimit: number;
}

export interface TickOutput {
  position: number;
  isFinished: boolean;
}

export function computeTickCommand(input: TickInput): TickOutput {
  const { position, isFinished } = interpolateKeyframes(
    input.keyframes,
    input.initialRad,
    input.elapsedSec,
    input.loop
  );

  const finalPosition = applyTrimAndClamp(
    position,
    input.trim,
    input.minLimit,
    input.maxLimit
  );

  return { position: finalPosition, isFinished };
}
