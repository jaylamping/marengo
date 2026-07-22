import type { AutoLearnRequest, AutoLearnResponse } from '@marengo/compound-auto-learn';

export type AutoLearnApiError =
  | { kind: 'not_configured'; message: string }
  | { kind: 'http'; status: number; message: string }
  | { kind: 'network'; message: string }
  | { kind: 'validation'; message: string; failures?: unknown };

export type AutoLearnApiResult =
  | { ok: true; response: AutoLearnResponse }
  | { ok: false; error: AutoLearnApiError };

function envTrim(key: 'VITE_AUTO_LEARN_URL' | 'VITE_AUTO_LEARN_TOKEN'): string | null {
  const value = (import.meta.env[key] as string | undefined)?.trim();
  return value || null;
}

export function autoLearnConfig(): {
  url: string | null;
  token: string | null;
} {
  return {
    url: envTrim('VITE_AUTO_LEARN_URL'),
    token: envTrim('VITE_AUTO_LEARN_TOKEN'),
  };
}

export function autoLearnConfigured(): boolean {
  const { url, token } = autoLearnConfig();
  return Boolean(url && token);
}

export async function postAutoLearn(
  request: AutoLearnRequest,
  signal?: AbortSignal,
): Promise<AutoLearnApiResult> {
  const { url, token } = autoLearnConfig();
  if (!url || !token) {
    return {
      ok: false,
      error: {
        kind: 'not_configured',
        message: 'Set VITE_AUTO_LEARN_URL and VITE_AUTO_LEARN_TOKEN',
      },
    };
  }

  try {
    const res = await fetch(`${url.replace(/\/$/, '')}/v1/auto-learn`, {
      method: 'POST',
      signal,
      headers: {
        'content-type': 'application/json',
        authorization: `Bearer ${token}`,
      },
      body: JSON.stringify(request),
    });
    const body = (await res.json().catch(() => ({}))) as Record<string, unknown>;
    if (!res.ok) {
      if (res.status === 400 && body.error === 'validation_failed') {
        return {
          ok: false,
          error: {
            kind: 'validation',
            message: 'Auto Learn response failed envelope checks',
            failures: body.failures,
          },
        };
      }
      return {
        ok: false,
        error: {
          kind: 'http',
          status: res.status,
          message:
            typeof body.error === 'string'
              ? body.error
              : `Auto Learn HTTP ${res.status}`,
        },
      };
    }
    return { ok: true, response: body as unknown as AutoLearnResponse };
  } catch (err) {
    if (signal?.aborted) {
      return {
        ok: false,
        error: { kind: 'network', message: 'Auto Learn request cancelled' },
      };
    }
    return {
      ok: false,
      error: {
        kind: 'network',
        message:
          err instanceof Error ? err.message : 'Could not reach Auto Learn BFF',
      },
    };
  }
}
