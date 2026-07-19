// @vitest-environment jsdom
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

function stubLocation(location: Partial<Location> & Pick<Location, 'protocol' | 'hostname' | 'origin'>) {
  vi.stubGlobal('location', location);
}

async function loadChappeConfig() {
  return import('@/lib/chappe-config');
}

describe('resolveChappeEndpoints', () => {
  beforeEach(() => {
    vi.stubEnv('VITE_CHAPPE_HTTP_URL', '');
    vi.stubEnv('VITE_CHAPPE_WEBTRANSPORT_URL', '');
  });

  afterEach(() => {
    vi.unstubAllEnvs();
    vi.unstubAllGlobals();
    vi.resetModules();
  });

  it('derives endpoints from HTTPS page origin when env vars are empty', async () => {
    stubLocation({
      protocol: 'https:',
      hostname: 'marengo.local',
      origin: 'https://marengo.local:8444',
    });

    const { resolveChappeEndpoints } = await loadChappeConfig();
    expect(resolveChappeEndpoints()).toEqual({
      endpoints: {
        httpUrl: 'https://marengo.local:8444',
        webTransportUrl: 'https://marengo.local:8443/chappe',
      },
      source: 'derived',
    });
  });

  it('prefers baked env URLs over robot-hosted derivation', async () => {
    vi.stubEnv('VITE_CHAPPE_HTTP_URL', 'http://127.0.0.1:8080/');
    vi.stubEnv('VITE_CHAPPE_WEBTRANSPORT_URL', 'https://127.0.0.1:8443/chappe/');
    stubLocation({
      protocol: 'https:',
      hostname: 'marengo.local',
      origin: 'https://marengo.local:8444',
    });

    const { resolveChappeEndpoints } = await loadChappeConfig();
    expect(resolveChappeEndpoints()).toEqual({
      endpoints: {
        httpUrl: 'http://127.0.0.1:8080',
        webTransportUrl: 'https://127.0.0.1:8443/chappe',
      },
      source: 'baked',
    });
  });

  it('returns none on HTTP pages without baked env', async () => {
    stubLocation({
      protocol: 'http:',
      hostname: 'localhost',
      origin: 'http://localhost:5173',
    });

    const { resolveChappeEndpoints, isChappeLive } = await loadChappeConfig();
    expect(resolveChappeEndpoints()).toEqual({ endpoints: null, source: 'none' });
    expect(isChappeLive()).toBe(false);
  });
});

describe('chappeMisconfigHint', () => {
  beforeEach(() => {
    vi.stubEnv('VITE_CHAPPE_HTTP_URL', '');
    vi.stubEnv('VITE_CHAPPE_WEBTRANSPORT_URL', '');
  });

  afterEach(() => {
    vi.unstubAllEnvs();
    vi.unstubAllGlobals();
    vi.resetModules();
  });

  it('flags baked localhost URLs on a remote HTTPS page', async () => {
    vi.stubEnv('VITE_CHAPPE_HTTP_URL', 'http://127.0.0.1:8080');
    vi.stubEnv('VITE_CHAPPE_WEBTRANSPORT_URL', 'https://127.0.0.1:8443/chappe');
    stubLocation({
      protocol: 'https:',
      hostname: 'marengo.local',
      origin: 'https://marengo.local:8444',
    });

    const { chappeMisconfigHint } = await loadChappeConfig();
    expect(chappeMisconfigHint()).toMatch(/127\.0\.0\.1|localhost/i);
    expect(chappeMisconfigHint()).toMatch(/baked|misconfig|redeploy/i);
  });

  it('treats localhost and 127.0.0.1 as equivalent loopback hosts', async () => {
    vi.stubEnv('VITE_CHAPPE_HTTP_URL', 'http://127.0.0.1:8080');
    vi.stubEnv('VITE_CHAPPE_WEBTRANSPORT_URL', 'https://127.0.0.1:8443/chappe');
    stubLocation({
      protocol: 'http:',
      hostname: 'localhost',
      origin: 'http://localhost:5173',
    });

    const { chappeMisconfigHint } = await loadChappeConfig();
    expect(chappeMisconfigHint()).toBeNull();
  });

  it('flags baked non-loopback host mismatch on page origin', async () => {
    vi.stubEnv('VITE_CHAPPE_HTTP_URL', 'http://gateway.example:8080');
    vi.stubEnv('VITE_CHAPPE_WEBTRANSPORT_URL', 'https://gateway.example:8443/chappe');
    stubLocation({
      protocol: 'http:',
      hostname: 'localhost',
      origin: 'http://localhost:5173',
    });

    const { chappeMisconfigHint } = await loadChappeConfig();
    expect(chappeMisconfigHint()).toMatch(/gateway\.example/);
    expect(chappeMisconfigHint()).toMatch(/localhost/);
  });

  it('returns null for origin-derived endpoints on HTTPS', async () => {
    stubLocation({
      protocol: 'https:',
      hostname: 'marengo.local',
      origin: 'https://marengo.local:8444',
    });

    const { chappeMisconfigHint } = await loadChappeConfig();
    expect(chappeMisconfigHint()).toBeNull();
  });
});

describe('chappeConnectionErrDetail', () => {
  it('includes endpoints and gateway-down hint for derived resolution', async () => {
    const { chappeConnectionErrDetail } = await loadChappeConfig();
    const detail = chappeConnectionErrDetail(
      {
        endpoints: {
          httpUrl: 'https://marengo.local:8444',
          webTransportUrl: 'https://marengo.local:8443/chappe',
        },
        source: 'derived',
      },
      null,
    );
    expect(detail).toContain('HTTP: https://marengo.local:8444');
    expect(detail).toContain('WebTransport: https://marengo.local:8443/chappe');
    expect(detail).toMatch(/gateway unreachable/i);
  });

  it('includes misconfig hint instead of gateway-down when baked host mismatches', async () => {
    const { chappeConnectionErrDetail } = await loadChappeConfig();
    const detail = chappeConnectionErrDetail(
      {
        endpoints: {
          httpUrl: 'http://127.0.0.1:8080',
          webTransportUrl: 'https://127.0.0.1:8443/chappe',
        },
        source: 'baked',
      },
      'Baked dev URLs (127.0.0.1) — redeploy Consul with production env scrub',
    );
    expect(detail).toContain('HTTP: http://127.0.0.1:8080');
    expect(detail).toContain('Baked dev URLs');
    expect(detail).not.toMatch(/gateway unreachable/i);
  });
});
