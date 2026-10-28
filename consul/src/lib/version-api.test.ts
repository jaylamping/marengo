import { afterEach, describe, expect, it, vi } from 'vitest';

import { setRuntimeCredential } from '@/lib/runtime-credentials';
import {
  clearSelfUpdateSession,
  fetchVersionStatus,
  readSelfUpdateSession,
  shasMatch,
  shortSha,
  writeSelfUpdateSession,
} from '@/lib/version-api';

vi.mock('@/lib/chappe-config', () => ({
  getChappeEndpoints: () => ({ httpUrl: 'https://gateway.fixture' }),
}));

describe('version-api helpers', () => {
  afterEach(() => {
    clearSelfUpdateSession();
    vi.unstubAllGlobals();
  });

  it('shasMatch accepts prefix and full equality', () => {
    expect(
      shasMatch(
        'abcdef0123456789abcdef0123456789abcdef01',
        'abcdef0',
      ),
    ).toBe(true);
    expect(shasMatch('aaaaaaa', 'bbbbbbb')).toBe(false);
    expect(shasMatch('', 'abcdef0')).toBe(false);
  });

  it('sends the management credential only with a forced refresh', async () => {
    setRuntimeCredential('management', 'mgmt-fixture');
    const fetchMock = vi.fn().mockResolvedValue(new Response('{}', { status: 200 }));
    vi.stubGlobal('fetch', fetchMock);
    await fetchVersionStatus({ refresh: true });
    await fetchVersionStatus();
    setRuntimeCredential('management', '');
    expect(fetchMock.mock.calls[0][0]).toBe('https://gateway.fixture/version/status?refresh=1');
    expect(fetchMock.mock.calls[0][1].headers).toEqual({
      Authorization: 'Bearer mgmt-fixture',
    });
    expect(fetchMock.mock.calls[1][0]).toBe('https://gateway.fixture/version/status');
    expect(fetchMock.mock.calls[1][1].headers).toBeUndefined();
  });

  it('shortSha truncates deploy-rev lines', () => {
    expect(shortSha('abcdef0123456789 2026-08-11T00:00:00Z')).toBe('abcdef0');
  });

  it('sessionStorage roundtrip for self-update watch bookmark', () => {
    writeSelfUpdateSession({
      jobId: 'j1',
      startedAtMs: 123,
      targetSha: 'bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb',
    });
    expect(readSelfUpdateSession()).toEqual({
      jobId: 'j1',
      startedAtMs: 123,
      targetSha: 'bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb',
    });
    clearSelfUpdateSession();
    expect(readSelfUpdateSession()).toBeNull();
  });
});
