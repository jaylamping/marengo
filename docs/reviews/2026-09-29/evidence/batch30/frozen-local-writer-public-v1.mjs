import assert from 'node:assert/strict';
import { createHash } from 'node:crypto';
import { mkdtemp, readFile, writeFile, chmod, rm } from 'node:fs/promises';
import { createServer } from 'node:net';
import os from 'node:os';
import path from 'node:path';
import { spawn } from 'node:child_process';
import { setTimeout as delay } from 'node:timers/promises';

const root = '/workspace';
const temporary = await mkdtemp(path.join(os.tmpdir(), 'marengo-local-writer-contract-'));
const sentinel = path.join(temporary, 'writer-invocations');
const writer = path.join(temporary, 'noop-writer');
await writeFile(writer, `#!/bin/sh\nprintf '%s\\n' invoked >> '${sentinel}'\nexit 0\n`);
await chmod(writer, 0o755);
const listener = createServer();
await new Promise(resolve => listener.listen(0, '127.0.0.1', resolve));
const port = listener.address().port;
await new Promise(resolve => listener.close(resolve));
const child = spawn(process.execPath, [path.join(root, 'tools/limit-sync-local/dist/server.js')], {
  env: { ...process.env, LIMIT_SYNC_PORT: String(port), LIMIT_SYNC_TOKEN: 'isolated-session-fixture', MARENGO_LIMIT_SYNC_BIN: writer },
  stdio: ['ignore', 'pipe', 'pipe'],
});
let output = '';
child.stdout.on('data', data => { output += data; });
child.stderr.on('data', data => { output += data; });
const exit = new Promise(resolve => child.once('exit', resolve));
const cases = [
  { name: 'foreign-origin simple request', origin: 'https://untrusted.example', type: 'text/plain', token: undefined, accepted: false },
  { name: 'allowed origin missing session credential', origin: 'http://localhost:5173', type: 'application/json', token: undefined, accepted: false },
  { name: 'allowed origin incorrect credential', origin: 'http://localhost:5173', type: 'application/json', token: 'incorrect-fixture', accepted: false },
  { name: 'allowed origin correct credential', origin: 'http://localhost:5173', type: 'application/json', token: 'isolated-session-fixture', accepted: true },
];
let failures = 0;
try {
  for (let tries = 0; tries < 100 && !output.includes('limit-sync-local on'); tries++) await delay(20);
  assert.match(output, /limit-sync-local on/, 'actual server must be ready');
  for (const test of cases) {
    const before = await readFile(sentinel, 'utf8').catch(() => '');
    const headers = { Origin: test.origin, 'Content-Type': test.type };
    if (test.token) headers.Authorization = `Bearer ${test.token}`;
    const response = await fetch(`http://127.0.0.1:${port}/local/limit-patch`, {
      method: 'POST', headers, body: JSON.stringify({ joint: 'right_elbow_pitch', lower: -1, upper: 1 }),
      signal: AbortSignal.timeout(5000),
    });
    const body = await response.text();
    const after = await readFile(sentinel, 'utf8').catch(() => '');
    try {
      if (test.accepted) {
        assert.equal(response.status, 200);
        assert.notEqual(after, before, 'valid request must reach noop writer');
      } else {
        assert.ok([401, 403].includes(response.status), 'unauthorized request must be refused');
        assert.equal(after, before, 'unauthorized request must never invoke writer');
      }
      console.log(JSON.stringify({ name: test.name, status: response.status, writer_invoked: after !== before, passed: true }));
    } catch (error) {
      failures++;
      console.log(JSON.stringify({ name: test.name, status: response.status, writer_invoked: after !== before, passed: false, error: error.message, body }));
    }
  }
  console.log(JSON.stringify({ methods: cases.length, failures, source_sha256: createHash('sha256').update(await readFile(path.join(root, 'tools/limit-sync-local/server.ts'))).digest('hex'), physical_device: false, real_config_writes: false }));
} finally {
  child.kill('SIGTERM');
  await Promise.race([exit, delay(5000).then(() => child.kill('SIGKILL'))]);
  await rm(temporary, { recursive: true, force: true });
}
process.exitCode = failures ? 1 : 0;
