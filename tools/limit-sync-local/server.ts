/** Loopback-only mirror of accepted Set Limits values into the local checkout. */
import { spawn } from "node:child_process";
import { randomBytes, timingSafeEqual } from "node:crypto";
import { createServer, type IncomingMessage, type ServerResponse } from "node:http";
import path from "node:path";
import { fileURLToPath } from "node:url";

const here = path.dirname(fileURLToPath(import.meta.url));
const pkgRoot = path.basename(here) === "dist" ? path.resolve(here, "..") : here;
const ROOT = path.resolve(pkgRoot, "../..");
const PORT = Number(process.env.LIMIT_SYNC_PORT || 8790);
const TOKEN = process.env.LIMIT_SYNC_TOKEN?.trim() || randomBytes(32).toString("base64url");
const MAX_BODY_BYTES = 16 * 1024;
const MAX_OUTPUT_BYTES = 64 * 1024;
const REQUEST_TIMEOUT_MS = 5000;
const WRITER_TIMEOUT_MS = 5000;
const MAX_REQUESTS_PER_MINUTE = 30;
const ALLOWED_ORIGINS = new Set(["http://localhost:5173", "http://127.0.0.1:5173"]);

class RequestFailure extends Error {
  constructor(readonly status: number, message: string) { super(message); }
}
function reply(res: ServerResponse, status: number, message: string): void {
  if (res.destroyed || res.writableEnded) return;
  res.writeHead(status, { "Content-Type": "application/json", "Cache-Control": "no-store" });
  res.end(JSON.stringify({ ok: status === 200, message }));
}
function authorized(req: IncomingMessage): boolean {
  const value = req.headers.authorization;
  if (!value?.startsWith("Bearer ")) return false;
  const supplied = Buffer.from(value.slice(7));
  const expected = Buffer.from(TOKEN);
  return supplied.length === expected.length && timingSafeEqual(supplied, expected);
}
function readBody(req: IncomingMessage): Promise<string> {
  return new Promise((resolve, reject) => {
    const chunks: Buffer[] = [];
    let bytes = 0;
    let settled = false;
    const timer = setTimeout(() => fail(new RequestFailure(408, "request body timeout")), REQUEST_TIMEOUT_MS);
    function fail(error: Error): void {
      if (settled) return;
      settled = true;
      clearTimeout(timer);
      reject(error);
    }
    req.on("data", (chunk: Buffer) => {
      if (settled) return;
      bytes += chunk.length;
      if (bytes > MAX_BODY_BYTES) {
        fail(new RequestFailure(413, "request body too large"));
        return;
      }
      chunks.push(chunk);
    });
    req.once("end", () => {
      if (settled) return;
      settled = true;
      clearTimeout(timer);
      try {
        resolve(new TextDecoder("utf-8", { fatal: true }).decode(Buffer.concat(chunks)));
      } catch {
        reject(new RequestFailure(400, "invalid UTF-8 JSON"));
      }
    });
    req.once("error", fail);
    req.once("aborted", () => fail(new RequestFailure(400, "request aborted")));
  });
}
function writerArguments(raw: string): string[] {
  let body: unknown;
  try { body = JSON.parse(raw); }
  catch { throw new RequestFailure(400, "invalid JSON"); }
  if (!body || typeof body !== "object" || Array.isArray(body)) {
    throw new RequestFailure(400, "invalid payload");
  }
  const { joint, lower, upper, soft_lower: softLower, soft_upper: softUpper } = body as Record<string, unknown>;
  if (typeof joint !== "string" || !/^[a-z][a-z0-9_]{0,63}$/.test(joint) ||
      typeof lower !== "number" || !Number.isFinite(lower) ||
      typeof upper !== "number" || !Number.isFinite(upper) || lower >= upper) {
    throw new RequestFailure(400, "invalid joint or hard bounds");
  }
  const args = ["--repo-root", ROOT, "--joint", joint, "--lower", String(lower), "--upper", String(upper)];
  if (softLower !== undefined || softUpper !== undefined) {
    if (typeof softLower !== "number" || !Number.isFinite(softLower) ||
        typeof softUpper !== "number" || !Number.isFinite(softUpper) ||
        softLower < lower || softUpper > upper || softLower > softUpper) {
      throw new RequestFailure(400, "invalid soft bounds");
    }
    args.push("--soft-lower", String(softLower), "--soft-upper", String(softUpper));
  }
  return args;
}
function runWriter(args: string[]): Promise<string> {
  const binary = process.env.MARENGO_LIMIT_SYNC_BIN ||
    path.join(ROOT, "target/debug/marengo-limit-sync" + (process.platform === "win32" ? ".exe" : ""));
  return new Promise((resolve, reject) => {
    const child = spawn(binary, args, { stdio: ["ignore", "pipe", "pipe"], windowsHide: true });
    const output: Buffer[] = [];
    let bytes = 0;
    let failure: RequestFailure | undefined;
    const timer = setTimeout(() => {
      failure = new RequestFailure(504, "local writer timeout");
      child.kill("SIGKILL");
    }, WRITER_TIMEOUT_MS);
    const collect = (chunk: Buffer): void => {
      if (failure) return;
      bytes += chunk.length;
      if (bytes > MAX_OUTPUT_BYTES) {
        failure = new RequestFailure(502, "local writer output too large");
        child.kill("SIGKILL");
      } else { output.push(chunk); }
    };
    child.stdout.on("data", collect);
    child.stderr.on("data", collect);
    child.once("error", () => {
      clearTimeout(timer);
      reject(new RequestFailure(502, "local writer could not start"));
    });
    child.once("close", status => {
      clearTimeout(timer);
      if (failure) reject(failure);
      else if (status !== 0) reject(new RequestFailure(502, "local writer failed"));
      else resolve(Buffer.concat(output).toString("utf8").trim() || "synced");
    });
  });
}

let busy = false;
let windowStarted = performance.now();
let windowRequests = 0;
const server = createServer(async (req, res) => {
  res.setHeader("Connection", "close");
  if (req.url !== "/local/limit-patch" || !["POST", "OPTIONS"].includes(req.method || "")) {
    reply(res, 404, "not found"); return;
  }
  const origin = req.headers.origin;
  if (!origin || !ALLOWED_ORIGINS.has(origin)) { reply(res, 403, "origin refused"); return; }
  res.setHeader("Access-Control-Allow-Origin", origin);
  res.setHeader("Vary", "Origin");
  res.setHeader("Access-Control-Allow-Methods", "POST, OPTIONS");
  res.setHeader("Access-Control-Allow-Headers", "Content-Type, Authorization");
  if (req.method === "OPTIONS") { res.writeHead(204); res.end(); return; }
  if (!authorized(req)) { reply(res, 401, "session credential required"); return; }
  if (req.headers["content-type"]?.split(";", 1)[0].trim().toLowerCase() !== "application/json") {
    reply(res, 415, "JSON content type required"); return;
  }
  if (busy) { reply(res, 503, "local writer busy"); return; }
  const now = performance.now();
  if (now - windowStarted >= 60_000) { windowStarted = now; windowRequests = 0; }
  if (windowRequests >= MAX_REQUESTS_PER_MINUTE) { reply(res, 429, "session request limit exceeded"); return; }
  windowRequests++;
  busy = true;
  try {
    const args = writerArguments(await readBody(req));
    reply(res, 200, await runWriter(args));
  } catch (error) {
    reply(res, error instanceof RequestFailure ? error.status : 500,
      error instanceof RequestFailure ? error.message : "local mirror failed");
  } finally { busy = false; }
});
server.requestTimeout = REQUEST_TIMEOUT_MS;
server.headersTimeout = REQUEST_TIMEOUT_MS;
server.maxHeadersCount = 32;
server.listen(PORT, "127.0.0.1", () => {
  const address = server.address();
  const port = address && typeof address !== "string" ? address.port : PORT;
  console.log(`limit-sync-local on http://127.0.0.1:${port} (repo ${ROOT})`);
  console.log(`Local mirror session credential: ${TOKEN}`);
});
