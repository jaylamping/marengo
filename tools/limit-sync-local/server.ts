/**
 * Loopback writer for Consul Set Limits → local git checkout.
 * Shells out to `marengo-limit-sync` (same Rust expand path as the Pi).
 *
 *   just limit-sync-serve
 *   # or: node tools/limit-sync-local/dist/server.js
 *
 * Consul: VITE_LIMIT_SYNC_URL=http://127.0.0.1:8790
 */
import { createServer, type IncomingMessage, type ServerResponse } from "node:http";
import { spawnSync } from "node:child_process";
import { fileURLToPath } from "node:url";
import path from "node:path";

const here = path.dirname(fileURLToPath(import.meta.url));
const pkgRoot = path.basename(here) === "dist" ? path.resolve(here, "..") : here;
const ROOT = path.resolve(pkgRoot, "../..");
const PORT = Number(process.env.LIMIT_SYNC_PORT || 8790);

function readBody(req: IncomingMessage): Promise<string> {
  return new Promise((resolve, reject) => {
    const chunks: Buffer[] = [];
    req.on("data", (c: Buffer) => chunks.push(c));
    req.on("end", () => resolve(Buffer.concat(chunks).toString("utf8")));
    req.on("error", reject);
  });
}

interface LimitPatchBody {
  joint?: unknown;
  lower?: unknown;
  upper?: unknown;
  soft_lower?: unknown;
  soft_upper?: unknown;
}

const ALLOWED_ORIGINS = new Set([
  "http://localhost:5173",
  "http://127.0.0.1:5173",
]);

const server = createServer(async (req: IncomingMessage, res: ServerResponse) => {
  const origin = req.headers.origin;
  if (origin && ALLOWED_ORIGINS.has(origin)) {
    res.setHeader("Access-Control-Allow-Origin", origin);
  }
  res.setHeader("Access-Control-Allow-Methods", "POST, OPTIONS");
  res.setHeader("Access-Control-Allow-Headers", "Content-Type");
  if (req.method === "OPTIONS") {
    res.writeHead(204);
    res.end();
    return;
  }
  if (req.method !== "POST" || req.url !== "/local/limit-patch") {
    res.writeHead(404, { "Content-Type": "application/json" });
    res.end(JSON.stringify({ ok: false, message: "not found" }));
    return;
  }
  try {
    const body = JSON.parse(await readBody(req)) as LimitPatchBody;
    const joint = String(body.joint || "");
    const lower = Number(body.lower);
    const upper = Number(body.upper);
    const softLower = body.soft_lower != null ? Number(body.soft_lower) : undefined;
    const softUpper = body.soft_upper != null ? Number(body.soft_upper) : undefined;
    if (!joint || !Number.isFinite(lower) || !Number.isFinite(upper)) {
      res.writeHead(400, { "Content-Type": "application/json" });
      res.end(JSON.stringify({ ok: false, message: "invalid payload" }));
      return;
    }
    if (
      (softLower != null && !Number.isFinite(softLower)) ||
      (softUpper != null && !Number.isFinite(softUpper))
    ) {
      res.writeHead(400, { "Content-Type": "application/json" });
      res.end(JSON.stringify({ ok: false, message: "invalid soft bounds" }));
      return;
    }
    if (joint.includes("..") || joint.includes("/") || joint.includes("\\")) {
      res.writeHead(400, { "Content-Type": "application/json" });
      res.end(JSON.stringify({ ok: false, message: "path rejected" }));
      return;
    }
    const bin =
      process.env.MARENGO_LIMIT_SYNC_BIN ||
      path.join(ROOT, "target/debug/marengo-limit-sync");
    const args = [
      "--repo-root",
      ROOT,
      "--joint",
      joint,
      "--lower",
      String(lower),
      "--upper",
      String(upper),
    ];
    if (Number.isFinite(softLower) && Number.isFinite(softUpper)) {
      args.push("--soft-lower", String(softLower), "--soft-upper", String(softUpper));
    }
    const run = spawnSync(bin, args, { encoding: "utf8" });
    if (run.status !== 0) {
      res.writeHead(500, { "Content-Type": "application/json" });
      res.end(
        JSON.stringify({
          ok: false,
          message: run.stderr || run.stdout || `exit ${run.status}`,
        }),
      );
      return;
    }
    res.writeHead(200, { "Content-Type": "application/json" });
    res.end(JSON.stringify({ ok: true, message: run.stderr || "synced" }));
  } catch (e) {
    res.writeHead(500, { "Content-Type": "application/json" });
    res.end(JSON.stringify({ ok: false, message: String(e) }));
  }
});

server.listen(PORT, "127.0.0.1", () => {
  console.log(`limit-sync-local on http://127.0.0.1:${PORT} (repo ${ROOT})`);
});
