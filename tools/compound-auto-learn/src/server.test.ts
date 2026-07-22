import { afterEach, describe, expect, it } from 'vitest';
import type { AutoLearnRequest } from '../shared/types';
import { createAutoLearnServer } from './server';

const token = 'test-token-xyz';

function sampleRequest(): AutoLearnRequest {
  return {
    presetId: 'arm_out_forward',
    stage: 'crawl',
    intent: 'test',
    operatorFeedback: null,
    joints: [
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
    ],
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
      movementBrief: 'Raise the arm forward to a held pose.',
      joints: ['right_shoulder_pitch', 'right_shoulder_roll'],
      teachKind: 'replace-program',
      keyframes: {},
    },
  };
}

const goodJson = JSON.stringify({
  stage: 'crawl',
  description: 'ok',
  landmarks: [
    {
      id: 'a',
      label: 'a',
      tSec: 0,
      q: { right_shoulder_pitch: 0.1, right_shoulder_roll: 0.05 },
      included: true,
    },
    {
      id: 'b',
      label: 'b',
      tSec: 4,
      q: { right_shoulder_pitch: 0.4, right_shoulder_roll: 0.2 },
      included: true,
    },
  ],
  cadenceScale: 1.5,
  settleDwellSec: 0.4,
  speedMultiplier: 0.35,
  source: 'auto_learn',
});

describe('auto-learn server', () => {
  let close: (() => Promise<void>) | null = null;

  afterEach(async () => {
    if (close) await close();
    close = null;
  });

  it('allows CORS preflight from Vite on :5174', async () => {
    const svc = createAutoLearnServer({
      token,
      port: 18786,
      promptFn: async () => goodJson,
    });
    await svc.listen();
    close = () => svc.close();
    const res = await fetch(`http://127.0.0.1:${svc.port}/v1/auto-learn`, {
      method: 'OPTIONS',
      headers: {
        origin: 'http://localhost:5174',
        'access-control-request-method': 'POST',
      },
    });
    expect(res.status).toBe(204);
    expect(res.headers.get('access-control-allow-origin')).toBe(
      'http://localhost:5174',
    );
  });

  it('rejects missing auth', async () => {
    const svc = createAutoLearnServer({
      token,
      port: 18787,
      promptFn: async () => goodJson,
    });
    await svc.listen();
    close = () => svc.close();
    const res = await fetch(`http://127.0.0.1:${svc.port}/v1/auto-learn`, {
      method: 'POST',
      headers: { 'content-type': 'application/json' },
      body: JSON.stringify(sampleRequest()),
    });
    expect(res.status).toBe(401);
  });

  it('returns validated response with bearer token', async () => {
    const svc = createAutoLearnServer({
      token,
      port: 18788,
      promptFn: async () => goodJson,
    });
    await svc.listen();
    close = () => svc.close();
    const res = await fetch(`http://127.0.0.1:${svc.port}/v1/auto-learn`, {
      method: 'POST',
      headers: {
        'content-type': 'application/json',
        authorization: `Bearer ${token}`,
      },
      body: JSON.stringify(sampleRequest()),
    });
    expect(res.status).toBe(200);
    const body = (await res.json()) as { stage: string; source: string };
    expect(body.stage).toBe('crawl');
    expect(body.source).toBe('auto_learn');
  });

  it('rejects walk without prior', async () => {
    const svc = createAutoLearnServer({
      token,
      port: 18789,
      promptFn: async () =>
        JSON.stringify({
          ...JSON.parse(goodJson),
          stage: 'walk',
          cadenceScale: 1,
          settleDwellSec: 0.25,
          speedMultiplier: 0.6,
        }),
    });
    await svc.listen();
    close = () => svc.close();
    const req = sampleRequest();
    req.stage = 'walk';
    const res = await fetch(`http://127.0.0.1:${svc.port}/v1/auto-learn`, {
      method: 'POST',
      headers: {
        'content-type': 'application/json',
        authorization: `Bearer ${token}`,
      },
      body: JSON.stringify(req),
    });
    expect(res.status).toBe(400);
  });
});
