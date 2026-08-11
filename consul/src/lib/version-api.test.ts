import { afterEach, describe, expect, it, vi } from 'vitest';

import {
  clearSelfUpdateSession,
  readSelfUpdateSession,
  shasMatch,
  shortSha,
  writeSelfUpdateSession,
} from '@/lib/version-api';

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

  it('shortSha truncates deploy-rev lines', () => {
    expect(shortSha('abcdef0123456789 2026-08-11T00:00:00Z')).toBe('abcdef0');
  });

  it('sessionStorage roundtrip for self-update resume', () => {
    writeSelfUpdateSession({
      jobId: 'j1',
      targetSha: 'abcdef0123456789',
      startedAtMs: 123,
    });
    expect(readSelfUpdateSession()).toEqual({
      jobId: 'j1',
      targetSha: 'abcdef0123456789',
      startedAtMs: 123,
    });
    clearSelfUpdateSession();
    expect(readSelfUpdateSession()).toBeNull();
  });
});
