// @vitest-environment jsdom
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

vi.mock('@/lib/chappe-config', () => ({
  getChappeEndpoints: vi.fn(() => ({
    httpUrl: 'http://test.local:8080',
    webTransportUrl: 'https://test.local:8443/chappe',
  })),
}));

vi.mock('@/lib/auto-learn-token', () => ({
  getAutoLearnOperatorToken: vi.fn(() => 'op-token-test'),
  setAutoLearnOperatorToken: vi.fn(),
  SESSION_KEY: 'marengo.autoLearnOperatorToken',
}));

import { getChappeEndpoints } from '@/lib/chappe-config';
import { getAutoLearnOperatorToken } from '@/lib/auto-learn-token';
import {
  autoLearnConfig,
  autoLearnConfigured,
  postAutoLearn,
} from '@/lib/auto-learn-api';
import type { AutoLearnRequest } from '@marengo/compound-auto-learn';

const mockGetChappeEndpoints = vi.mocked(getChappeEndpoints);
const mockGetToken = vi.mocked(getAutoLearnOperatorToken);

const minimalRequest = {
  schemaVersion: 1,
  stage: 'crawl',
} as unknown as AutoLearnRequest;

function mockResponse(status: number, body: unknown = {}): Response {
  return {
    ok: status >= 200 && status < 300,
    status,
    json: () => Promise.resolve(body),
  } as Response;
}

describe('autoLearnConfig / autoLearnConfigured', () => {
  beforeEach(() => {
    mockGetChappeEndpoints.mockReturnValue({
      httpUrl: 'http://test.local:8080',
      webTransportUrl: 'https://test.local:8443/chappe',
    });
    mockGetToken.mockReturnValue('op-token-test');
  });

  afterEach(() => {
    vi.restoreAllMocks();
  });

  it('is configured when chappe httpUrl and operator token are set', () => {
    expect(autoLearnConfig()).toEqual({
      url: 'http://test.local:8080/v1/auto-learn',
      token: 'op-token-test',
    });
    expect(autoLearnConfigured()).toBe(true);
  });

  it('is not configured without token', () => {
    mockGetToken.mockReturnValue(null);
    expect(autoLearnConfigured()).toBe(false);
    expect(autoLearnConfig().token).toBeNull();
  });

  it('is not configured without chappe httpUrl', () => {
    mockGetChappeEndpoints.mockReturnValue(null);
    expect(autoLearnConfigured()).toBe(false);
    expect(autoLearnConfig().url).toBeNull();
  });
});

describe('postAutoLearn', () => {
  beforeEach(() => {
    mockGetChappeEndpoints.mockReturnValue({
      httpUrl: 'http://test.local:8080',
      webTransportUrl: 'https://test.local:8443/chappe',
    });
    mockGetToken.mockReturnValue('op-token-test');
  });

  afterEach(() => {
    vi.restoreAllMocks();
  });

  it('POSTs with x-marengo-auto-learn-token (not Bearer)', async () => {
    const fetchSpy = vi.fn().mockResolvedValue(mockResponse(200, { ok: true }));
    vi.stubGlobal('fetch', fetchSpy);

    await postAutoLearn(minimalRequest);

    expect(fetchSpy).toHaveBeenCalledTimes(1);
    const [url, init] = fetchSpy.mock.calls[0] as [string, RequestInit];
    expect(url).toBe('http://test.local:8080/v1/auto-learn');
    expect(init.method).toBe('POST');
    const headers = init.headers as Record<string, string>;
    expect(headers['x-marengo-auto-learn-token']).toBe('op-token-test');
    expect(headers.authorization).toBeUndefined();
    expect(headers.Authorization).toBeUndefined();
  });

  it('returns not_configured without token', async () => {
    mockGetToken.mockReturnValue(null);
    const fetchSpy = vi.fn();
    vi.stubGlobal('fetch', fetchSpy);

    const result = await postAutoLearn(minimalRequest);

    expect(result.ok).toBe(false);
    if (!result.ok) {
      expect(result.error.kind).toBe('not_configured');
    }
    expect(fetchSpy).not.toHaveBeenCalled();
  });

  it('maps 503 auto_learn_unavailable', async () => {
    vi.stubGlobal(
      'fetch',
      vi.fn().mockResolvedValue(
        mockResponse(503, { error: 'auto_learn_unavailable' }),
      ),
    );

    const result = await postAutoLearn(minimalRequest);

    expect(result.ok).toBe(false);
    if (!result.ok && result.error.kind === 'http') {
      expect(result.error.status).toBe(503);
      expect(result.error.message).toMatch(/unavailable/i);
    }
  });

  it('maps 401 unauthorized', async () => {
    vi.stubGlobal('fetch', vi.fn().mockResolvedValue(mockResponse(401, {})));

    const result = await postAutoLearn(minimalRequest);

    expect(result.ok).toBe(false);
    if (!result.ok && result.error.kind === 'http') {
      expect(result.error.status).toBe(401);
      expect(result.error.message).toMatch(/operator token/i);
    }
  });
});
