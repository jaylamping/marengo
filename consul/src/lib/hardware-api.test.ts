import { afterEach, describe, expect, it, vi } from 'vitest';

import { getChappeEndpoints } from '@/lib/chappe-config';
import { activateUrdf, fetchCompleteness } from '@/lib/hardware-api';

vi.mock('@/lib/chappe-config', () => ({
  getChappeEndpoints: vi.fn(),
}));

afterEach(() => {
  vi.clearAllMocks();
  vi.unstubAllGlobals();
});

describe('hardware API failures', () => {
  it('rejects completeness when gateway endpoints are unavailable', async () => {
    vi.mocked(getChappeEndpoints).mockReturnValue(null);

    await expect(fetchCompleteness()).rejects.toThrow('Chappe endpoints not configured');
  });

  it('rejects completeness when the gateway returns an error', async () => {
    vi.mocked(getChappeEndpoints).mockReturnValue({
      httpUrl: 'http://gateway.test',
      webTransportUrl: 'https://gateway.test/chappe',
    });
    vi.stubGlobal(
      'fetch',
      vi.fn(async () => new Response(null, { status: 503 })),
    );

    await expect(fetchCompleteness()).rejects.toThrow(
      'Hardware completeness failed: HTTP 503',
    );
  });

  it('surfaces a JSON conflict message when activation returns HTTP 409', async () => {
    vi.mocked(getChappeEndpoints).mockReturnValue({
      httpUrl: 'http://gateway.test',
      webTransportUrl: 'https://gateway.test/chappe',
    });
    vi.stubGlobal(
      'fetch',
      vi.fn(
        async () =>
          new Response(
            JSON.stringify({
              error: 'Disable motors before activating a URDF.',
            }),
            {
              status: 409,
              headers: { 'content-type': 'application/json' },
            },
          ),
      ),
    );

    await expect(
      activateUrdf({
        upload_id: 'upload-conflict',
        resolutions: [],
      }),
    ).rejects.toThrow('Disable motors before activating a URDF.');
  });
});
