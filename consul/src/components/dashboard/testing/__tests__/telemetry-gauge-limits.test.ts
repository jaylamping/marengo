import { describe, expect, it } from 'vitest';

import { resolveGaugeLimits } from '../telemetry-gauge-grid';

describe('resolveGaugeLimits', () => {
  it('never invents per-motor-type caps when nothing is published', () => {
    // Baseline showed tau_ff 60 Nm / 50 rad/s (rs03 table) and ±π here.
    const limits = resolveGaugeLimits({
      live: null,
      envelope: null,
      benchTorqueNm: undefined,
      benchPosition: undefined,
      configVelocityMaxRadS: undefined,
    });
    expect(limits).toEqual({
      torqueLimitNm: null,
      velocityMaxRadS: null,
      position: null,
    });
  });

  it('prefers the live Davout snapshot, then the master config', () => {
    const fromConfig = resolveGaugeLimits({
      live: null,
      envelope: null,
      benchTorqueNm: 5,
      benchPosition: { lower: -0.9, upper: 2.9 },
      configVelocityMaxRadS: 2.5,
    });
    expect(fromConfig).toEqual({
      torqueLimitNm: 5,
      velocityMaxRadS: 2.5,
      position: { lower: -0.9, upper: 2.9 },
    });
    const fromLive = resolveGaugeLimits({
      live: { tauFfMaxNm: 3, velocityMaxRadS: 2 },
      envelope: { hardLowerRad: -0.5, hardUpperRad: 0.5 },
      benchTorqueNm: 5,
      benchPosition: { lower: -0.9, upper: 2.9 },
      configVelocityMaxRadS: 2.5,
    });
    expect(fromLive).toEqual({
      torqueLimitNm: 3,
      velocityMaxRadS: 2,
      position: { lower: -0.5, upper: 0.5 },
    });
  });

  it('treats zero, non-finite and inverted values as unknown', () => {
    const limits = resolveGaugeLimits({
      live: { tauFfMaxNm: 0, velocityMaxRadS: Number.NaN },
      envelope: null,
      benchTorqueNm: undefined,
      benchPosition: { lower: 1, upper: -1 },
      configVelocityMaxRadS: undefined,
    });
    expect(limits).toEqual({
      torqueLimitNm: null,
      velocityMaxRadS: null,
      position: null,
    });
  });
});
