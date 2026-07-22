#!/usr/bin/env node
/**
 * Loopback writer for Consul Set Limits → local git checkout.
 * Shells out to `marengo-limit-sync` (same Rust expand path as the Pi).
 *
 *   just limit-sync-serve
 *   # or: node tools/limit-sync-local/server.mjs
 *
 * Consul: VITE_LIMIT_SYNC_URL=http://127.0.0.1:8790
 */
import { createServer } from 'node:http';
import { spawnSync } from 'node:child_process';
import { fileURLToPath } from 'node:url';
import path from 'node:path';

const ROOT = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../..');
const PORT = Number(process.env.LIMIT_SYNC_PORT || 8790);
const ALLOWED = new Set([
  'arm_4dof_right',
  'arm_3dof_right',
  'arm_4dof',
  'shoulder_pitch_dual',
  'shoulder_pitch_right',
  'shoulder_pitch_left',
]);

function readBody(req) {
  return new Promise((resolve, reject) => {
    const chunks = [];
    req.on('data', (c) => chunks.push(c));
    req.on('end', () => resolve(Buffer.concat(chunks).toString('utf8')));
    req.on('error', reject);
  });
}

const server = createServer(async (req, res) => {
  res.setHeader('Access-Control-Allow-Origin', 'http://localhost:5173');
  res.setHeader('Access-Control-Allow-Methods', 'POST, OPTIONS');
  res.setHeader('Access-Control-Allow-Headers', 'Content-Type');
  if (req.method === 'OPTIONS') {
    res.writeHead(204);
    res.end();
    return;
  }
  if (req.method !== 'POST' || req.url !== '/local/limit-patch') {
    res.writeHead(404, { 'Content-Type': 'application/json' });
    res.end(JSON.stringify({ ok: false, message: 'not found' }));
    return;
  }
  try {
    const body = JSON.parse(await readBody(req));
    const profile = String(body.profile || '');
    const joint = String(body.joint || '');
    const lower = Number(body.lower);
    const upper = Number(body.upper);
    const softLower = body.soft_lower != null ? Number(body.soft_lower) : undefined;
    const softUpper = body.soft_upper != null ? Number(body.soft_upper) : undefined;
    if (!ALLOWED.has(profile) || !joint || !Number.isFinite(lower) || !Number.isFinite(upper)) {
      res.writeHead(400, { 'Content-Type': 'application/json' });
      res.end(JSON.stringify({ ok: false, message: 'invalid payload' }));
      return;
    }
    if (
      (softLower != null && !Number.isFinite(softLower)) ||
      (softUpper != null && !Number.isFinite(softUpper))
    ) {
      res.writeHead(400, { 'Content-Type': 'application/json' });
      res.end(JSON.stringify({ ok: false, message: 'invalid soft bounds' }));
      return;
    }
    if (profile.includes('..') || joint.includes('..') || joint.includes('/')) {
      res.writeHead(400, { 'Content-Type': 'application/json' });
      res.end(JSON.stringify({ ok: false, message: 'path rejected' }));
      return;
    }
    const bin =
      process.env.MARENGO_LIMIT_SYNC_BIN ||
      path.join(ROOT, 'target/debug/marengo-limit-sync');
    const args = [
      '--repo-root',
      ROOT,
      '--profile',
      profile,
      '--joint',
      joint,
      '--lower',
      String(lower),
      '--upper',
      String(upper),
    ];
    if (Number.isFinite(softLower) && Number.isFinite(softUpper)) {
      args.push('--soft-lower', String(softLower), '--soft-upper', String(softUpper));
    }
    const run = spawnSync(bin, args, { encoding: 'utf8' });
    if (run.status !== 0) {
      res.writeHead(500, { 'Content-Type': 'application/json' });
      res.end(
        JSON.stringify({
          ok: false,
          message: run.stderr || run.stdout || `exit ${run.status}`,
        }),
      );
      return;
    }
    res.writeHead(200, { 'Content-Type': 'application/json' });
    res.end(JSON.stringify({ ok: true, message: run.stderr || 'synced' }));
  } catch (e) {
    res.writeHead(500, { 'Content-Type': 'application/json' });
    res.end(JSON.stringify({ ok: false, message: String(e) }));
  }
});

server.listen(PORT, '127.0.0.1', () => {
  console.log(`limit-sync-local on http://127.0.0.1:${PORT} (repo ${ROOT})`);
});
