import type { AutoLearnRequest, AutoLearnResponse } from '@marengo/compound-auto-learn';
import { getChappeEndpoints } from '@/lib/chappe-config';
import { getAutoLearnOperatorToken } from '@/lib/auto-learn-token';

export type AutoLearnApiError =
  | { kind: 'not_configured'; message: string }
  | { kind: 'http'; status: number; message: string }
  | { kind: 'network'; message: string }
  | { kind: 'validation'; message: string; failures?: unknown };

export type AutoLearnApiResult =
  | { ok: true; response: AutoLearnResponse }
  | { ok: false; error: AutoLearnApiError };

const NOT_CONFIGURED_MESSAGE =
  'Set Auto Learn operator token (Vite VITE_AUTO_LEARN_OPERATOR_TOKEN or paste in panel) and ensure gateway is reachable';

export function autoLearnConfig(): {
  url: string | null;
  token: string | null;
} {
  const httpUrl = getChappeEndpoints()?.httpUrl ?? null;
  const url = httpUrl ? `${httpUrl}/v1/auto-learn` : null;
  return {
    url,
    token: getAutoLearnOperatorToken(),
  };
}

export function autoLearnConfigured(): boolean {
  const { url, token } = autoLearnConfig();
  return Boolean(url && token);
}

function httpErrorMessage(status: number, body: Record<string, unknown>): string {
  const err = typeof body.error === 'string' ? body.error : '';
  if (status === 401) {
    return 'Unauthorized — Auto Learn operator token mismatch (x-marengo-auto-learn-token vs MARENGO_AUTO_LEARN_OPERATOR_TOKEN)';
  }
  if (status === 503) {
    if (err === 'auto_learn_upstream_token_missing') {
      return 'Auto Learn upstream token missing on Pi (set AUTO_LEARN_TOKEN for compound-auto-learn)';
    }
    if (err === 'auto_learn_unavailable') {
      return 'Auto Learn unavailable on Pi — is compound-auto-learn running behind the gateway?';
    }
    return err || 'Auto Learn unavailable (HTTP 503)';
  }
  return err || `Auto Learn HTTP ${status}`;
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
        message: NOT_CONFIGURED_MESSAGE,
      },
    };
  }

  try {
    const res = await fetch(url, {
      method: 'POST',
      signal,
      headers: {
        'content-type': 'application/json',
        'x-marengo-auto-learn-token': token,
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
          message: httpErrorMessage(res.status, body),
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
          err instanceof Error ? err.message : 'Could not reach Auto Learn via gateway',
      },
    };
  }
}
