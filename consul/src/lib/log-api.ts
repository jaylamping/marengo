import { getChappeEndpoints } from '@/lib/chappe-config';

export type StructuredLogEntryDto = {
  id: number;
  timestamp_ms: number;
  level: string;
  target: string;
  message: string;
  session_id: string;
  fields_json: string;
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
  offset_s: number;
  interface: string;
  can_id: string;
  data: string;
  line_no: number;
  timestamp_unix_us?: number;
  comm_type?: number;
  comm_type_name?: string;
  joint?: string;
};

export type LogErrorKind =
  | 'no_endpoint'
  | 'unauthorized'
  | 'not_found'
  | 'unavailable'
  | 'server'
  | 'network';

export type LogApiError = { kind: LogErrorKind; status?: number };

export type LogApiResult<T> =
  | { ok: true; data: T }
  | { ok: false; error: LogApiError };

export type AsyncSlice<T> = {
  loading: boolean;
  error: LogApiError | null;
  data: T;
};

export type CandumpIdCountDto = {
  can_id: string;
  count: number;
};

export type CandumpInterfaceSummaryDto = {
  name: string;
  parsed_frames: number;
  approx_hz: number | null;
};

export type CandumpSummaryDto = {
  parsed_frames: number;
  total_lines: number;
  source_bytes: number;
  duration_s: number;
  approx_hz: number | null;
  interfaces: CandumpInterfaceSummaryDto[];
  top_ids: CandumpIdCountDto[];
  frame_count?: number;
  bytes?: number;
};

function statusToKind(status: number): LogErrorKind {
  if (status === 401) {
    return 'unauthorized';
  }
  if (status === 404) {
    return 'not_found';
  }
  if (status === 503) {
    return 'unavailable';
  }
  if (status >= 500) {
    return 'server';
  }
  return 'server';
}

function baseUrl(): string | null {
  return getChappeEndpoints()?.httpUrl ?? null;
}

// Blob endpoints (bench/trace/candump bodies) may map store failures to HTTP 404,
// so `not_found` can conflate "missing object" with upstream store errors.
async function logFetch<T>(path: string): Promise<LogApiResult<T>> {
  const root = baseUrl();
  if (!root) {
    return { ok: false, error: { kind: 'no_endpoint' } };
  }
  const token = import.meta.env.VITE_MARENGO_LOG_TOKEN as string | undefined;
  const headers: Record<string, string> = {};
  if (token?.trim()) {
    headers['x-marengo-log-token'] = token.trim();
  }
  try {
    const res = await fetch(`${root}${path}`, { headers });
    if (!res.ok) {
      return { ok: false, error: { kind: statusToKind(res.status), status: res.status } };
    }
    const data = (await res.json()) as T;
    return { ok: true, data };
  } catch {
    return { ok: false, error: { kind: 'network' } };
  }
}

export function shouldShowLogErrorBanner(error: LogApiError | null | undefined): boolean {
  return error != null && error.kind !== 'no_endpoint';
}

export function logErrorMessage(error: LogApiError): string {
  switch (error.kind) {
    case 'unauthorized':
      return 'Log request rejected: gateway token invalid or required.';
    case 'not_found':
      return 'Log resource not found.';
    case 'unavailable':
      return 'Log store temporarily unavailable.';
    case 'server':
      return 'Log gateway returned a server error.';
    case 'network':
      return 'Could not reach the log gateway.';
    case 'no_endpoint':
      return '';
  }
}

export async function fetchRecentLogs(
  limit = 5000,
): Promise<LogApiResult<StructuredLogEntryDto[]>> {
  const result = await logFetch<{ entries: StructuredLogEntryDto[] }>(
    `/snapshot/logs/recent?limit=${limit}`,
  );
  if (!result.ok) {
    return result;
  }
  return { ok: true, data: result.data.entries };
}

export async function fetchSessions(limit = 50): Promise<LogApiResult<LogSessionDto[]>> {
  const result = await logFetch<{ sessions: LogSessionDto[] }>(`/logs/sessions?limit=${limit}`);
  if (!result.ok) {
    return result;
  }
  return { ok: true, data: result.data.sessions };
}

export async function fetchStructuredLogs(params: {
  q?: string;
  level?: string;
  offset?: number;
  limit?: number;
}): Promise<LogApiResult<{ entries: StructuredLogEntryDto[]; total: number }>> {
  const search = new URLSearchParams();
  if (params.q) search.set('q', params.q);
  if (params.level) search.set('level', params.level);
  if (params.offset !== undefined) search.set('offset', String(params.offset));
  search.set('limit', String(params.limit ?? 200));
  return logFetch<{ entries: StructuredLogEntryDto[]; total: number }>(
    `/logs/structured?${search.toString()}`,
  );
}

export async function fetchCandumpPage(
  sessionId: string | 'latest',
  offset = 0,
  limit = 200,
): Promise<LogApiResult<{ frames: CandumpFrameDto[]; total_frames: number; parsed_frames?: number }>> {
  const path =
    sessionId === 'latest'
      ? `/logs/sessions/latest/candump?offset=${offset}&limit=${limit}`
      : `/logs/sessions/${encodeURIComponent(sessionId)}/candump?offset=${offset}&limit=${limit}`;
  return logFetch<{ frames: CandumpFrameDto[]; total_frames: number; parsed_frames?: number }>(path);
}

export async function fetchCandumpSummary(
  sessionId: string | 'latest',
): Promise<LogApiResult<CandumpSummaryDto>> {
  const path =
    sessionId === 'latest'
      ? '/logs/sessions/latest/candump/summary'
      : `/logs/sessions/${encodeURIComponent(sessionId)}/candump/summary`;
  return logFetch<CandumpSummaryDto>(path);
}

export async function fetchBenchLines(
  sessionId: string,
  offset = 0,
  limit = 200,
): Promise<LogApiResult<{ lines: string[]; total: number }>> {
  return logFetch<{ lines: string[]; total: number }>(
    `/logs/sessions/${encodeURIComponent(sessionId)}/bench?offset=${offset}&limit=${limit}`,
  );
}

export async function fetchTraceLines(
  sessionId: string,
  offset = 0,
  limit = 200,
): Promise<LogApiResult<{ lines: string[]; total: number }>> {
  return logFetch<{ lines: string[]; total: number }>(
    `/logs/sessions/${encodeURIComponent(sessionId)}/trace?offset=${offset}&limit=${limit}`,
  );
}
