// @vitest-environment jsdom
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { cleanup, renderHook, waitFor } from '@testing-library/react';

import { useActiveReportingLease } from '@/hooks/use-active-reporting-lease';

type LeaseArgs = {
  joint: string;
  clientId: string;
  leaseId: string;
  action: 'acquire' | 'renew' | 'release';
  keepalive?: boolean;
};

const postLease = vi.fn(async (_args: LeaseArgs) => undefined);

vi.mock('@/lib/gateway-api', () => ({
  postActiveReportingLease: (args: LeaseArgs) => postLease(args),
}));

vi.mock('@/lib/chappe-config', () => ({
  isChappeLive: () => true,
}));

vi.mock('@/state/actuatorStore', () => ({
  ensureClientId: () => 'client-test',
}));

afterEach(() => {
  cleanup();
  postLease.mockReset();
  postLease.mockImplementation(async () => undefined);
});

beforeEach(() => {
  vi.stubGlobal('crypto', {
    randomUUID: () => 'lease-fixed-id',
  });
});

describe('useActiveReportingLease', () => {
  it('acquires on enable and releases on cleanup', async () => {
    const { unmount } = renderHook(() =>
      useActiveReportingLease({ joint: 'right_shoulder_pitch', enabled: true }),
    );

    await waitFor(() => {
      expect(postLease).toHaveBeenCalledWith(
        expect.objectContaining({
          joint: 'right_shoulder_pitch',
          action: 'acquire',
          leaseId: 'lease-fixed-id',
          clientId: 'client-test',
        }),
      );
    });

    unmount();

    await waitFor(() => {
      expect(postLease).toHaveBeenCalledWith(
        expect.objectContaining({
          joint: 'right_shoulder_pitch',
          action: 'release',
          leaseId: 'lease-fixed-id',
        }),
      );
    });
  });

  it('stays idle when disabled', () => {
    const { result } = renderHook(() =>
      useActiveReportingLease({ joint: 'right_shoulder_pitch', enabled: false }),
    );
    expect(result.current).toBe('idle');
    expect(postLease).not.toHaveBeenCalled();
  });
});
