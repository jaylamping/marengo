// @vitest-environment node
import { afterEach, describe, expect, it, vi } from 'vitest';

import { LOG_CONTEXT_MAX_BYTES } from '@marengo/compound-auto-learn';
import { buildAutoLearnLogContext } from '@/lib/auto-learn-logs';

vi.mock('@/lib/log-api', () => ({
  fetchStructuredLogs: vi.fn(),
  fetchRecentLogs: vi.fn(),
}));

import { fetchRecentLogs, fetchStructuredLogs } from '@/lib/log-api';

const mockedStructured = vi.mocked(fetchStructuredLogs);
const mockedRecent = vi.mocked(fetchRecentLogs);

afterEach(() => {
  vi.clearAllMocks();
});

describe('buildAutoLearnLogContext', () => {
  it('returns failure when both log APIs fail', async () => {
    mockedStructured.mockResolvedValue({
      ok: false,
      error: { kind: 'unavailable', status: 503 },
    });
    mockedRecent.mockResolvedValue({
      ok: false,
      error: { kind: 'unavailable', status: 503 },
    });
    const result = await buildAutoLearnLogContext(null);
    expect(result.ok).toBe(false);
  });

  it('truncates oversized summaries', async () => {
    const huge = 'x'.repeat(LOG_CONTEXT_MAX_BYTES + 200);
    mockedStructured.mockResolvedValue({
      ok: true,
      data: {
        entries: [
          {
            id: 1,
            timestamp_ms: 1,
            level: 'INFO',
            target: 'davout',
            message: huge,
            session_id: 's',
            fields_json: '{}',
          },
        ],
        total: 1,
      },
    });
    const result = await buildAutoLearnLogContext(null);
    expect(result.ok).toBe(true);
    if (result.ok) {
      expect(result.context.truncated).toBe(true);
      expect(new TextEncoder().encode(result.context.summaryText).length).toBeLessThanOrEqual(
        LOG_CONTEXT_MAX_BYTES,
      );
    }
  });

  it('filters by sinceMs', async () => {
    mockedStructured.mockResolvedValue({
      ok: true,
      data: {
        entries: [
          {
            id: 1,
            timestamp_ms: 10,
            level: 'INFO',
            target: 'a',
            message: 'old',
            session_id: 's',
            fields_json: '{}',
          },
          {
            id: 2,
            timestamp_ms: 100,
            level: 'WARN',
            target: 'b',
            message: 'new',
            session_id: 's',
            fields_json: '{}',
          },
        ],
        total: 2,
      },
    });
    const result = await buildAutoLearnLogContext(50);
    expect(result.ok).toBe(true);
    if (result.ok) {
      expect(result.context.summaryText).toContain('new');
      expect(result.context.summaryText).not.toContain(' old');
    }
  });
});
