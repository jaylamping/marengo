import { getChappeEndpoints } from '@/lib/chappe-config';
import { parseDeployRev } from '@/lib/host-debug-info';

export type DeployJobState = 'idle' | 'running' | 'succeeded' | 'failed';

export type DeployJobDto = {
  state: DeployJobState;
  job_id: string;
  target_sha: string;
  result_sha: string;
  unit_name: string;
  started_at: string;
  updated_at: string;
  message: string;
  phase: string;
};

export type VersionStatusDto = {
  deploy_sha: string;
  deployed_at?: string | null;
  upstream_sha: string;
  upstream_fetched_at?: string | null;
  upstream_ok: boolean;
  update_available: boolean;
  ready_for_target: boolean;
  deploy: DeployJobDto;
  log_tail?: string | null;
};

export type DeployResponseDto = {
  ok: boolean;
  message: string;
  already_current?: boolean;
  job_id?: string;
  target_sha?: string;
};

const SESSION_KEY = 'consul.selfUpdate';

export type SelfUpdateSession = {
  jobId: string;
  targetSha: string;
  startedAtMs: number;
};

function baseUrl(): string | null {
  return getChappeEndpoints()?.httpUrl ?? null;
}

function authHeaders(): Record<string, string> {
  const headers: Record<string, string> = {
    'Content-Type': 'application/json',
  };
  const token = import.meta.env.VITE_MARENGO_LOG_TOKEN as string | undefined;
  if (token?.trim()) {
    headers['x-marengo-log-token'] = token.trim();
  }
  return headers;
}

export function shasMatch(installed: string, upstream: string): boolean {
  const a = installed.trim().toLowerCase();
  const b = upstream.trim().toLowerCase();
  if (!a || !b) return false;
  if (a.length >= 7 && b.length >= 7) {
    return a === b || a.startsWith(b) || b.startsWith(a);
  }
  return a === b;
}

export function shortSha(sha: string): string {
  const { rev } = parseDeployRev(sha);
  return rev.slice(0, 7) || '—';
}

export function readSelfUpdateSession(): SelfUpdateSession | null {
  try {
    const raw = sessionStorage.getItem(SESSION_KEY);
    if (!raw) return null;
    const parsed = JSON.parse(raw) as SelfUpdateSession;
    if (!parsed?.jobId || !parsed?.targetSha) return null;
    return parsed;
  } catch {
    return null;
  }
}

export function writeSelfUpdateSession(session: SelfUpdateSession): void {
  sessionStorage.setItem(SESSION_KEY, JSON.stringify(session));
}

export function clearSelfUpdateSession(): void {
  sessionStorage.removeItem(SESSION_KEY);
}

export async function fetchVersionStatus(options?: {
  refresh?: boolean;
  signal?: AbortSignal;
}): Promise<VersionStatusDto | null> {
  const root = baseUrl();
  if (!root) return null;
  const qs = options?.refresh ? '?refresh=1' : '';
  try {
    const res = await fetch(`${root}/version/status${qs}`, {
      signal: options?.signal,
    });
    if (!res.ok) return null;
    return (await res.json()) as VersionStatusDto;
  } catch {
    return null;
  }
}

export async function startSelfDeploy(init?: {
  signal?: AbortSignal;
}): Promise<DeployResponseDto> {
  const root = baseUrl();
  if (!root) {
    return { ok: false, message: 'Chappe HTTP URL not configured' };
  }
  try {
    const res = await fetch(`${root}/control/deploy`, {
      method: 'POST',
      headers: authHeaders(),
      body: JSON.stringify({ confirm: true }),
      signal: init?.signal,
    });
    const text = await res.text();
    let parsed: DeployResponseDto | null = null;
    try {
      parsed = JSON.parse(text) as DeployResponseDto;
    } catch {
      parsed = null;
    }
    if (parsed && typeof parsed.ok === 'boolean') {
      return {
        ...parsed,
        ok: parsed.ok && (res.ok || res.status === 202),
        message: parsed.message || res.statusText || `HTTP ${res.status}`,
      };
    }
    return {
      ok: false,
      message: text.trim() || `HTTP ${res.status}`,
    };
  } catch (e) {
    return {
      ok: false,
      message: e instanceof Error ? e.message : 'Deploy request failed',
    };
  }
}

/** Client-side update poll timeout (native Pi build can be long). */
export const SELF_UPDATE_TIMEOUT_MS = 20 * 60 * 1000;
