import { getChappeEndpoints } from '@/lib/chappe-config';

export type StructuredLogEntryDto = {
  id: number;
  timestamp_ms: number;
  level: string;
  target: string;
  message: string;
  session_id: string;
};

export type LogSessionDto = {
  id: string;
  label: string;
  started_ms: number;
  ended_ms: number;
  has_bench: boolean;
  has_candump: boolean;
  has_trace: boolean;
  candump_bytes: number;
  candump_frame_count: number;
};

export type CandumpFrameDto = {
  delta_s: number;
  interface: string;
  can_id: string;
  data: string;
  line_no: number;
};

function baseUrl(): string | null {
  return getChappeEndpoints()?.httpUrl ?? null;
}

async function logFetch<T>(path: string): Promise<T | null> {
  const root = baseUrl();
  if (!root) {
    return null;
  }
  const token = import.meta.env.VITE_MARENGO_LOG_TOKEN as string | undefined;
  const headers: Record<string, string> = {};
  if (token?.trim()) {
    headers['x-marengo-log-token'] = token.trim();
  }
  const res = await fetch(`${root}${path}`, { headers });
  if (!res.ok) {
    return null;
  }
  return (await res.json()) as T;
}

export async function fetchRecentLogs(limit = 5000): Promise<StructuredLogEntryDto[]> {
  const data = await logFetch<{ entries: StructuredLogEntryDto[] }>(
    `/snapshot/logs/recent?limit=${limit}`,
  );
  return data?.entries ?? [];
}

export async function fetchSessions(limit = 50): Promise<LogSessionDto[]> {
  const data = await logFetch<{ sessions: LogSessionDto[] }>(
    `/logs/sessions?limit=${limit}`,
  );
  return data?.sessions ?? [];
}

export async function fetchStructuredLogs(params: {
  q?: string;
  level?: string;
  offset?: number;
  limit?: number;
}): Promise<{ entries: StructuredLogEntryDto[]; total: number }> {
  const search = new URLSearchParams();
  if (params.q) search.set('q', params.q);
  if (params.level) search.set('level', params.level);
  if (params.offset !== undefined) search.set('offset', String(params.offset));
  search.set('limit', String(params.limit ?? 200));
  const data = await logFetch<{ entries: StructuredLogEntryDto[]; total: number }>(
    `/logs/structured?${search.toString()}`,
  );
  return data ?? { entries: [], total: 0 };
}

export async function fetchCandumpPage(
  sessionId: string | 'latest',
  offset = 0,
  limit = 200,
): Promise<{ frames: CandumpFrameDto[]; total_frames: number }> {
  const path =
    sessionId === 'latest'
      ? `/logs/sessions/latest/candump?offset=${offset}&limit=${limit}`
      : `/logs/sessions/${encodeURIComponent(sessionId)}/candump?offset=${offset}&limit=${limit}`;
  const data = await logFetch<{ frames: CandumpFrameDto[]; total_frames: number }>(path);
  return data ?? { frames: [], total_frames: 0 };
}

export async function fetchCandumpSummary(sessionId: string) {
  return logFetch<{
    frame_count: number;
    bytes: number;
    duration_s: number;
    approx_hz: number;
    interfaces: string[];
    top_ids: string[];
  }>(`/logs/sessions/${encodeURIComponent(sessionId)}/candump/summary`);
}

export async function fetchBenchLines(
  sessionId: string,
  offset = 0,
  limit = 200,
): Promise<{ lines: string[]; total: number }> {
  const data = await logFetch<{ lines: string[]; total: number }>(
    `/logs/sessions/${encodeURIComponent(sessionId)}/bench?offset=${offset}&limit=${limit}`,
  );
  return data ?? { lines: [], total: 0 };
}

export async function fetchTraceLines(
  sessionId: string,
  offset = 0,
  limit = 200,
): Promise<{ lines: string[]; total: number }> {
  const data = await logFetch<{ lines: string[]; total: number }>(
    `/logs/sessions/${encodeURIComponent(sessionId)}/trace?offset=${offset}&limit=${limit}`,
  );
  return data ?? { lines: [], total: 0 };
}
