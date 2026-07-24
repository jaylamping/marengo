// @vitest-environment jsdom
import { afterEach, describe, expect, it, vi } from 'vitest';

import {
  fetchBenchLines,
  fetchCandumpPage,
  fetchCandumpSummary,
  fetchRecentLogs,
  fetchSessions,
  fetchStructuredLogs,
  fetchTraceLines,
  type LogErrorKind,
} from '@/lib/log-api';

vi.mock('@/lib/chappe-config', () => ({
  getChappeEndpoints: vi.fn(() => ({
    httpUrl: 'http://test.local',
    webTransportUrl: 'https://test.local:8443/chappe',
  })),
}));

import { getChappeEndpoints } from '@/lib/chappe-config';

const mockGetChappeEndpoints = vi.mocked(getChappeEndpoints);

function mockResponse(status: number, body: unknown = {}): Response {
  return {
    ok: status >= 200 && status < 300,
    status,
    json: () => Promise.resolve(body),
  } as Response;
}

describe('logFetch status mapping', () => {
  afterEach(() => {
    vi.restoreAllMocks();
    mockGetChappeEndpoints.mockReturnValue({
      httpUrl: 'http://test.local',
      webTransportUrl: 'https://test.local:8443/chappe',
    });
  });

  it.each<[number, LogErrorKind]>([
    [401, 'unauthorized'],
    [404, 'not_found'],
    [503, 'unavailable'],
    [500, 'server'],
    [502, 'server'],
  ])('maps HTTP %i to %s', async (status, kind) => {
    vi.stubGlobal('fetch', vi.fn().mockResolvedValue(mockResponse(status)));

    const result = await fetchSessions();

    expect(result).toEqual({ ok: false, error: { kind, status } });
  });

  it('returns no_endpoint when Chappe URL is unset', async () => {
    mockGetChappeEndpoints.mockReturnValue(null);
    const fetchSpy = vi.fn();
    vi.stubGlobal('fetch', fetchSpy);

    const result = await fetchSessions();

    expect(result).toEqual({ ok: false, error: { kind: 'no_endpoint' } });
    expect(fetchSpy).not.toHaveBeenCalled();
  });

  it('maps thrown fetch to network', async () => {
    vi.stubGlobal('fetch', vi.fn().mockRejectedValue(new Error('offline')));

    const result = await fetchSessions();

    expect(result).toEqual({ ok: false, error: { kind: 'network' } });
  });

  it('maps invalid JSON to network', async () => {
    vi.stubGlobal(
      'fetch',
      vi.fn().mockResolvedValue({
        ok: true,
        status: 200,
        json: () => Promise.reject(new SyntaxError('bad json')),
      }),
    );

    const result = await fetchSessions();

    expect(result).toEqual({ ok: false, error: { kind: 'network' } });
  });
});

describe('fetch* helpers preserve failures', () => {
  afterEach(() => {
    vi.restoreAllMocks();
    mockGetChappeEndpoints.mockReturnValue({
      httpUrl: 'http://test.local',
      webTransportUrl: 'https://test.local:8443/chappe',
    });
  });

  it.each([
    ['fetchRecentLogs', () => fetchRecentLogs()],
    ['fetchSessions', () => fetchSessions()],
    ['fetchStructuredLogs', () => fetchStructuredLogs({})],
    ['fetchCandumpPage', () => fetchCandumpPage('latest')],
    ['fetchCandumpSummary', () => fetchCandumpSummary('sess-1')],
    ['fetchCandumpSummaryLatest', () => fetchCandumpSummary('latest')],
    ['fetchBenchLines', () => fetchBenchLines('sess-1')],
    ['fetchTraceLines', () => fetchTraceLines('sess-1')],
  ] as const)('%s returns ok:false on HTTP failure', async (_name, run) => {
    vi.stubGlobal('fetch', vi.fn().mockResolvedValue(mockResponse(503)));

    const result = await run();

    expect(result.ok).toBe(false);
    if (!result.ok) {
      expect(result.error.kind).toBe('unavailable');
    }
  });
});
