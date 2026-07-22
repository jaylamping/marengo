import { createServer, type IncomingMessage, type ServerResponse } from 'node:http';
import { resolve as pathResolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import { defaultPromptFn, runAutoLearnPrompt, type PromptFn } from './agent';
import { parseAndAssertResponse, parseAutoLearnRequest } from './parse';

const DEFAULT_PORT = 8787;
const BODY_LIMIT = 256 * 1024;
const RATE_WINDOW_MS = 60_000;
const RATE_MAX = 20;
/** Local Vite only (port may bump when 5173 is taken). */
function isAllowedCorsOrigin(origin: string | undefined): origin is string {
  if (!origin) return false;
  let url: URL;
  try {
    url = new URL(origin);
  } catch {
    return false;
  }
  if (url.protocol !== 'http:') return false;
  if (url.hostname !== '127.0.0.1' && url.hostname !== 'localhost') {
    return false;
  }
  const port = Number(url.port || '80');
  return port >= 5173 && port <= 5199;
}

type RateBucket = { count: number; resetAt: number };

export type ServerOptions = {
  port?: number;
  host?: string;
  token?: string;
  promptFn?: PromptFn;
};

function readBody(req: IncomingMessage, limit: number): Promise<string> {
  return new Promise((resolve, reject) => {
    const chunks: Buffer[] = [];
    let size = 0;
    req.on('data', (chunk: Buffer) => {
      size += chunk.length;
      if (size > limit) {
        reject(new Error('body_too_large'));
        req.destroy();
        return;
      }
      chunks.push(chunk);
    });
    req.on('end', () => resolve(Buffer.concat(chunks).toString('utf8')));
    req.on('error', reject);
  });
}

function sendJson(
  res: ServerResponse,
  status: number,
  body: unknown,
  origin: string | undefined,
): void {
  const headers: Record<string, string> = {
    'content-type': 'application/json; charset=utf-8',
  };
  if (isAllowedCorsOrigin(origin)) {
    headers['access-control-allow-origin'] = origin;
    headers['access-control-allow-headers'] = 'authorization, content-type';
    headers['access-control-allow-methods'] = 'POST, OPTIONS';
    headers.vary = 'Origin';
  }
  res.writeHead(status, headers);
  res.end(status === 204 ? '' : JSON.stringify(body));
}

function authorize(req: IncomingMessage, token: string): boolean {
  const header = req.headers.authorization ?? '';
  if (header === `Bearer ${token}`) return true;
  const alt = req.headers['x-auto-learn-token'];
  return typeof alt === 'string' && alt === token;
}

export function createAutoLearnServer(opts: ServerOptions = {}) {
  const token = (opts.token ?? process.env.AUTO_LEARN_TOKEN ?? '').trim();
  const host = opts.host ?? '127.0.0.1';
  const port = opts.port ?? Number(process.env.PORT ?? DEFAULT_PORT);
  const promptFn = opts.promptFn ?? defaultPromptFn();
  const buckets = new Map<string, RateBucket>();

  if (!token) {
    throw new Error('AUTO_LEARN_TOKEN is required');
  }

  const server = createServer(async (req, res) => {
    const origin = req.headers.origin;
    if (req.method === 'OPTIONS') {
      sendJson(res, 204, {}, origin);
      return;
    }

    if (req.method !== 'POST' || req.url !== '/v1/auto-learn') {
      sendJson(res, 404, { error: 'not_found' }, origin);
      return;
    }

    if (!authorize(req, token)) {
      sendJson(res, 401, { error: 'unauthorized' }, origin);
      return;
    }

    const now = Date.now();
    const bucket = buckets.get('local') ?? {
      count: 0,
      resetAt: now + RATE_WINDOW_MS,
    };
    if (now > bucket.resetAt) {
      bucket.count = 0;
      bucket.resetAt = now + RATE_WINDOW_MS;
    }
    bucket.count += 1;
    buckets.set('local', bucket);
    if (bucket.count > RATE_MAX) {
      sendJson(res, 429, { error: 'rate_limited' }, origin);
      return;
    }

    try {
      const raw = await readBody(req, BODY_LIMIT);
      const body = JSON.parse(raw) as unknown;
      const request = parseAutoLearnRequest(body);

      let modelText = await runAutoLearnPrompt(request, promptFn);
      let parsed = parseAndAssertResponse(request, modelText);
      if (!parsed.ok) {
        const hint = parsed.failures
          .map((f) => `${f.code}: ${f.message}`)
          .join('\n');
        modelText = await runAutoLearnPrompt(request, promptFn, hint);
        parsed = parseAndAssertResponse(request, modelText);
      }
      if (!parsed.ok) {
        sendJson(
          res,
          400,
          { error: 'validation_failed', failures: parsed.failures },
          origin,
        );
        return;
      }
      sendJson(res, 200, parsed.response, origin);
    } catch (err) {
      if (err instanceof Error && err.message === 'body_too_large') {
        sendJson(res, 413, { error: 'body_too_large' }, origin);
        return;
      }
      const message = err instanceof Error ? err.message : 'server_error';
      const status = message.includes('CURSOR_API_KEY') ? 503 : 400;
      sendJson(res, status, { error: message }, origin);
    }
  });

  return {
    listen: () =>
      new Promise<void>((resolve) => {
        server.listen(port, host, () => resolve());
      }),
    close: () =>
      new Promise<void>((resolve, reject) => {
        server.close((err) => (err ? reject(err) : resolve()));
      }),
    port,
    host,
    server,
  };
}

async function main(): Promise<void> {
  if (!process.env.CURSOR_API_KEY?.trim()) {
    console.error('CURSOR_API_KEY is required');
    process.exit(1);
  }
  const svc = createAutoLearnServer();
  await svc.listen();
  console.log(
    `compound-auto-learn listening on http://${svc.host}:${svc.port}/v1/auto-learn`,
  );
}

const entry = process.argv[1] ? pathResolve(process.argv[1]) : '';
if (entry && fileURLToPath(import.meta.url) === entry) {
  void main();
}
