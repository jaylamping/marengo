// @vitest-environment jsdom
import { act, renderHook, waitFor } from '@testing-library/react';
import { describe, expect, it, vi } from 'vitest';

import { useArchiveSessions } from '@/hooks/use-archive-sessions';
import { fetchBenchLines } from '@/lib/log-api';

vi.mock('@/lib/chappe-config', () => ({ isChappeLive: () => false }));
vi.mock('@/lib/log-buffer', () => ({
  ensureLogsSeeded: () => {},
  hydrateLogsFromSnapshot: () => {},
  setLogsPageActive: () => {},
  SNAPSHOT_HYDRATE_LIMIT: 300,
}));
vi.mock('@/lib/log-api', () => ({
  fetchSessions: vi.fn(async () => ({ ok: true, data: [] })),
  fetchRecentLogs: vi.fn(),
  fetchBenchLines: vi.fn(),
  fetchTraceLines: vi.fn(),
}));

type Lines = { ok: true; data: { lines: string[]; total: number } };

describe('archive session lines (UI-19)', () => {
  it('a slow response for a session no longer selected is dropped', async () => {
    let resolveSlow: (value: Lines) => void = () => {};
    vi.mocked(fetchBenchLines).mockImplementation((session) =>
      session === 'slow'
        ? new Promise<Lines>((resolve) => { resolveSlow = resolve; })
        : Promise.resolve({ ok: true, data: { lines: ['fast line'], total: 1 } }),
    );
    const { result } = renderHook(() => useArchiveSessions('archive'));
    act(() => result.current.setSelectedSession('slow'));
    act(() => result.current.setSelectedSession('fast'));
    await waitFor(() => expect(result.current.linesState.data).toEqual(['fast line']));
    await act(async () => {
      resolveSlow({ ok: true, data: { lines: ['slow line'], total: 1 } });
    });
    expect(result.current.linesState.data).toEqual(['fast line']);
  });

  it('reports the session total so a truncated view can say so', async () => {
    vi.mocked(fetchBenchLines).mockResolvedValue({
      ok: true,
      data: { lines: ['l1', 'l2'], total: 9000 },
    });
    const { result } = renderHook(() => useArchiveSessions('archive'));
    act(() => result.current.setSelectedSession('long'));
    await waitFor(() => expect(result.current.linesTotal).toBe(9000));
  });
});
