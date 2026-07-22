import { describe, expect, it } from 'vitest';
import {
  assertAutoLearnResponse,
  assertCrawlFirst,
  assertMaterializedSchedule,
  materializeSegmentDurationsSec,
  normalizeOperatorFeedback,
} from './asserts';
import type { AutoLearnRequest, AutoLearnResponse } from './types';

function baseRequest(
  overrides: Partial<AutoLearnRequest> = {},
): AutoLearnRequest {
  const joints = [
    {
      name: 'right_shoulder_pitch',
      positionRad: 0,
      velocityRadS: 0,
      torqueNm: 0,
      tempC: 25,
      positionLowerRad: -0.9,
      positionUpperRad: 3.25,
      velocityMaxRadS: 2,
      torqueLimitNm: 15,
    },
    {
      name: 'right_shoulder_roll',
      positionRad: 0,
      velocityRadS: 0,
      torqueNm: 0,
      tempC: 25,
      positionLowerRad: -0.85,
      positionUpperRad: 3.14,
      velocityMaxRadS: 2,
      torqueLimitNm: 15,
    },
  ];
  return {
    presetId: 'arm_out_forward',
    stage: 'crawl',
    intent: 'test',
    operatorFeedback: null,
    joints,
    livePositions: {
      right_shoulder_pitch: 0,
      right_shoulder_roll: 0,
    },
    priorLandmarks: null,
    priorDescription: null,
    logContext: null,
    base: {
      name: 'Arm Out',
      description: 'test',
      joints: ['right_shoulder_pitch', 'right_shoulder_roll'],
      teachKind: 'replace-program',
      keyframes: {},
    },
    ...overrides,
  };
}

function okResponse(
  request: AutoLearnRequest,
  overrides: Partial<AutoLearnResponse> = {},
): AutoLearnResponse {
  return {
    stage: request.stage,
    description: 'crawl raise',
    landmarks: [
      {
        id: 'lm0',
        label: 'start',
        tSec: 0,
        q: {
          right_shoulder_pitch: 0.1,
          right_shoulder_roll: 0.05,
        },
        included: true,
      },
      {
        id: 'lm1',
        label: 'end',
        tSec: 4,
        q: {
          right_shoulder_pitch: 0.5,
          right_shoulder_roll: 0.3,
        },
        included: true,
      },
    ],
    cadenceScale: 1.5,
    settleDwellSec: 0.4,
    speedMultiplier: 0.35,
    source: 'auto_learn',
    ...overrides,
  };
}

describe('assertCrawlFirst', () => {
  it('rejects non-crawl without prior', () => {
    const r = assertCrawlFirst('walk', null);
    expect(r.ok).toBe(false);
  });

  it('allows crawl without prior', () => {
    expect(assertCrawlFirst('crawl', null).ok).toBe(true);
  });
});

describe('assertAutoLearnResponse', () => {
  it('accepts a slow crawl draft', () => {
    const req = baseRequest();
    const res = okResponse(req);
    expect(assertAutoLearnResponse(req, res).ok).toBe(true);
  });

  it('rejects live→first slam', () => {
    const req = baseRequest();
    const res = okResponse(req, {
      landmarks: [
        {
          id: 'lm0',
          label: 'far',
          tSec: 0,
          q: {
            right_shoulder_pitch: 3.0,
            right_shoulder_roll: 0.05,
          },
          included: true,
        },
        {
          id: 'lm1',
          label: 'end',
          tSec: 4,
          q: {
            right_shoulder_pitch: 3.1,
            right_shoulder_roll: 0.1,
          },
          included: true,
        },
      ],
    });
    const result = assertAutoLearnResponse(req, res);
    expect(result.ok).toBe(false);
    if (!result.ok) {
      expect(result.failures.some((f) => f.code === 'live_first_step')).toBe(
        true,
      );
    }
  });

  it('rejects cadence below floor', () => {
    const req = baseRequest();
    const res = okResponse(req, { cadenceScale: 0.5 });
    const result = assertAutoLearnResponse(req, res);
    expect(result.ok).toBe(false);
  });

  it('requires 3 landmarks for wave teachKind', () => {
    const req = baseRequest({
      presetId: 'wave',
      base: {
        name: 'Wave',
        description: 'wave',
        joints: ['right_shoulder_pitch', 'right_shoulder_roll'],
        teachKind: 'replace-native-wave',
        keyframes: {},
      },
    });
    const res = okResponse(req);
    const result = assertAutoLearnResponse(req, res);
    expect(result.ok).toBe(false);
    if (!result.ok) {
      expect(result.failures.some((f) => f.code === 'landmark_count')).toBe(
        true,
      );
    }
  });
});

describe('normalizeOperatorFeedback', () => {
  it('nulls whitespace', () => {
    expect(normalizeOperatorFeedback('  \n')).toBeNull();
  });

  it('keeps short notes', () => {
    expect(normalizeOperatorFeedback(' too fast on pitch ')).toBe(
      'too fast on pitch',
    );
  });
});

describe('materializeSegmentDurationsSec', () => {
  it('golden: approach + cadence×Δt + dwell, then / speedMultiplier', () => {
    const landmarks = [
      {
        id: 'a',
        label: 'a',
        tSec: 0,
        q: { right_shoulder_pitch: 0, right_shoulder_roll: 0 },
        included: true,
      },
      {
        id: 'b',
        label: 'b',
        tSec: 2,
        q: { right_shoulder_pitch: 0.2, right_shoulder_roll: 0.1 },
        included: true,
      },
    ];
    const cadenceScale = 1.5;
    const settleDwellSec = 0.4;
    const durations = materializeSegmentDurationsSec(
      landmarks,
      cadenceScale,
      settleDwellSec,
    );
    // i=0 approach: max(0.5, 0.5*scale + dwell); i=1: max(0.15, Δt*scale) + dwell
    expect(durations).toEqual([
      Math.max(0.5, 0.5 * cadenceScale + settleDwellSec),
      Math.max(0.15, (2 - 0) * cadenceScale) + settleDwellSec,
    ]);
    const speedMultiplier = 0.35;
    const effectiveTail = durations![1]! / speedMultiplier;
    expect(effectiveTail).toBeCloseTo(3.4 / 0.35, 6);
    expect(
      assertMaterializedSchedule(
        landmarks,
        cadenceScale,
        settleDwellSec,
        speedMultiplier,
        'crawl',
      ).ok,
    ).toBe(true);
  });
});
