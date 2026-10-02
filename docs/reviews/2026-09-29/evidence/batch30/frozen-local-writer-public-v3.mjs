import assert from 'node:assert/strict';
import { spawn } from 'node:child_process';
import { chmod, copyFile, mkdir, mkdtemp, readFile, rm } from 'node:fs/promises';
import { request } from 'node:http';
import os from 'node:os';
import path from 'node:path';
import { after, before, test } from 'node:test';
import { setTimeout as delay } from 'node:timers/promises';
import { fileURLToPath } from 'node:url';

const packageRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
const repoRoot = path.resolve(packageRoot, '../..');
const credential = 'isolated-session-contract';
const valid = { joint: 'right_elbow_pitch', lower: -1.25, upper: 1.5,
  soft_lower: -1.2, soft_upper: 1.45 };
let suite;
let writer;
let realWriter;
const fixtureFiles = ['config/robot.yaml', 'config/motors.yaml', 'config/control.yaml',
  'config/homing.yaml', 'assets/urdf/marengo.urdf'];

function run(command, args, cwd = repoRoot) {
  return new Promise((resolve, reject) => {
    const child = spawn(command, args, { cwd, windowsHide: true });
    let output = '';
    child.stdout.on('data', data => { output += data; });
    child.stderr.on('data', data => { output += data; });
    child.once('error', reject);
    child.once('close', status => status === 0 ? resolve(output) : reject(new Error(output)));
  });
}

async function removeOwnedFixture(directory) {
  const resolved = path.resolve(directory);
  const base = path.resolve(suite);
  assert.ok(resolved === base || resolved.startsWith(base + path.sep));
  assert.ok(base.startsWith(path.join(path.resolve(os.tmpdir()), 'marengo-limit-sync-contract-')));
  await rm(resolved, { recursive: true, force: true });
}

before(async () => {
  suite = await mkdtemp(path.join(os.tmpdir(), 'marengo-limit-sync-contract-'));
  writer = path.join(suite, process.platform === 'win32' ? 'writer.exe' : 'writer');
  await run('rustc', ['--edition=2021', path.join(packageRoot, 'test/writer-fixture.rs'), '-o', writer]);
  if (process.platform !== 'win32') await chmod(writer, 0o755);
  await run('cargo', ['build', '--locked', '-p', 'marengo-limit-sync']);
  const metadata = JSON.parse(await run('cargo', ['metadata', '--no-deps', '--format-version', '1']));
  realWriter = path.join(metadata.target_directory, 'debug',
    process.platform === 'win32' ? 'marengo-limit-sync.exe' : 'marengo-limit-sync');
});
after(async () => { if (suite) await removeOwnedFixture(suite); });

async function startServer({ generated = false, binary = writer, root = repoRoot, configOverride } = {}) {
  const directory = await mkdtemp(path.join(suite, 'server-'));
  const log = path.join(directory, 'invocations.jsonl');
  const environment = { ...process.env, LIMIT_SYNC_PORT: '0', LIMIT_SYNC_TOKEN: credential,
    MARENGO_LIMIT_SYNC_BIN: binary, WRITER_FIXTURE_LOG: log };
  if (configOverride) environment.MARENGO_CONFIG_DIR = configOverride;
  if (generated) delete environment.LIMIT_SYNC_TOKEN;
  const child = spawn(process.execPath, [path.join(root, 'tools/limit-sync-local/dist/server.js')], {
    env: environment, windowsHide: true, stdio: ['ignore', 'pipe', 'pipe'],
  });
  let output = '';
  child.stdout.on('data', data => { output += data; });
  child.stderr.on('data', data => { output += data; });
  let ended = false;
  let startupError;
  const exit = new Promise(resolve => {
    child.once('exit', () => { ended = true; resolve(); });
    child.once('error', error => { startupError = error; ended = true; resolve(); });
  });
  for (let attempt = 0; attempt < 250 && !output.includes('Local mirror session credential:') && !ended; attempt++) {
    await delay(20);
  }
  if (startupError) throw startupError;
  const url = output.match(/limit-sync-local on (http:\/\/127\.0\.0\.1:\d+)/)?.[1];
  const token = output.match(/Local mirror session credential: ([^\r\n]+)/)?.[1];
  assert.ok(url && token, 'actual server must bind loopback and finish startup');
  async function calls() {
    const lines = await readFile(log, 'utf8').catch(() => '');
    return lines.trim() ? lines.trim().split('\n').map(line => JSON.parse(line)) : [];
  }
  async function stop() {
    if (!ended) child.kill('SIGTERM');
    await Promise.race([exit, delay(2000).then(() => { if (!ended) child.kill('SIGKILL'); })]);
    await exit;
    // Failed deadline assertions may leave the stand-in running. It exits by
    // itself within ten seconds; never signal a PID read from a stale log.
    for (const invocation of await calls()) {
      for (let attempt = 0; attempt < 550; attempt++) {
        try { process.kill(invocation.pid, 0); } catch { break; }
        await delay(20);
        if (attempt === 549) throw new Error('owned writer fixture did not finish');
      }
    }
    await removeOwnedFixture(directory);
  }
  async function post({ payload = valid, body, origin = 'http://localhost:5173',
    auth = token, type = 'application/json' } = {}) {
    const headers = {};
    if (origin !== null) headers.Origin = origin;
    if (auth !== null) headers.Authorization = `Bearer ${auth}`;
    if (type !== null) headers['Content-Type'] = type;
    const response = await fetch(url + '/local/limit-patch', {
      method: 'POST', headers, body: body ?? JSON.stringify(payload), signal: AbortSignal.timeout(8000),
    });
    return { status: response.status, body: await response.json(), headers: response.headers };
  }
  return { url, token, post, calls, stop };
}

test('actual routes refuse origin, credential, content type and invalid payloads before invoking writer',
  { timeout: 15000 }, async t => {
    const server = await startServer();
    try {
      const cases = [
        ['foreign simple POST', { origin: 'https://untrusted.example', type: 'text/plain' }, 403],
        ['missing origin', { origin: null }, 403],
        ['missing credential', { auth: null }, 401],
        ['wrong credential', { auth: 'different-session-contract' }, 401],
        ['text/plain with correct credential', { type: 'text/plain' }, 415],
        ['missing content type', { type: null }, 415],
        ['malformed JSON', { body: '{' }, 400],
        ['array body', { payload: [] }, 400],
        ['null body', { payload: null }, 400],
        ['string bound', { payload: { ...valid, lower: '-1.25' } }, 400],
        ['missing bound', { payload: { joint: valid.joint, upper: 1.5 } }, 400],
        ['reversed hard bounds', { payload: { ...valid, lower: 2 } }, 400],
        ['only one soft bound', { payload: { joint: valid.joint, lower: -1, upper: 1, soft_lower: -0.9 } }, 400],
        ['soft outside hard', { payload: { ...valid, soft_upper: 2 } }, 400],
        ['path joint', { payload: { ...valid, joint: '../outside' } }, 400],
        ['oversized JSON', { payload: { ...valid, padding: 'x'.repeat(20000) } }, 413],
        ['invalid UTF-8 JSON', { body: Buffer.from([123,34,120,34,58,34,255,34,125]) }, 400],
      ];
      for (const [name, options, status] of cases) {
        await t.test(name, async () => {
          const response = await server.post(options);
          assert.equal(response.status, status);
          assert.equal(response.body.ok, false);
          assert.deepEqual(await server.calls(), [], 'refused payload must not invoke writer');
        });
      }
      await t.test('preflight permits only the approved origin and credential header', async () => {
        const response = await fetch(server.url + '/local/limit-patch', {
          method: 'OPTIONS', headers: { Origin: 'http://localhost:5173',
            'Access-Control-Request-Headers': 'Authorization, Content-Type' },
        });
        assert.equal(response.status, 204);
        assert.equal(response.headers.get('access-control-allow-origin'), 'http://localhost:5173');
        assert.match(response.headers.get('access-control-allow-headers'), /Authorization/);
        assert.deepEqual(await server.calls(), []);
      });
      const response = await server.post();
      assert.equal(response.status, 200);
      const invocations = await server.calls();
      assert.equal(invocations.length, 1);
      assert.deepEqual(invocations[0].args, ['--repo-root', repoRoot, '--joint', valid.joint,
        '--lower', '-1.25', '--upper', '1.5', '--soft-lower', '-1.2', '--soft-upper', '1.45']);
    } finally { await server.stop(); }
  });

test('a stalled actual worker leaves HTTP responsive, refuses overlap and is reaped before retry',
  { timeout: 20000 }, async () => {
    const server = await startServer();
    try {
      const started = performance.now();
      const pending = server.post({ payload: { ...valid, joint: 'right_worker_stall' } });
      for (let attempt = 0; attempt < 100 && (await server.calls()).length === 0; attempt++) await delay(20);
      assert.equal((await server.calls()).length, 1, 'actual fixture must be running');
      const probe = performance.now();
      assert.equal((await server.post()).status, 503);
      assert.ok(performance.now() - probe < 1000, 'spawn must not block HTTP');
      const response = await pending;
      assert.equal(response.status, 504);
      assert.ok(performance.now() - started < 7500, 'worker deadline must be bounded');
      const [{ pid }] = await server.calls();
      assert.throws(() => process.kill(pid, 0), 'timed-out fixture must be reaped');
      assert.equal((await server.post()).status, 200, 'retry must get a new bounded worker');
    } finally { await server.stop(); }
  });

test('a stalled request body expires without invoking writer and then releases admission',
  { timeout: 15000 }, async () => {
    const server = await startServer();
    let connection;
    try {
      const started = performance.now();
      const response = new Promise((resolve, reject) => {
        connection = request(server.url + '/local/limit-patch', { method: 'POST', headers: {
          Origin: 'http://localhost:5173', Authorization: `Bearer ${server.token}`,
          'Content-Type': 'application/json', 'Transfer-Encoding': 'chunked',
        } }, message => {
          message.resume();
          message.once('end', () => resolve(message.statusCode));
        });
        connection.once('error', reject);
        connection.write('{');
      });
      assert.equal(await response, 408);
      assert.ok(performance.now() - started < 7500);
      assert.deepEqual(await server.calls(), []);
      assert.equal((await server.post()).status, 200);
    } finally { connection?.destroy(); await server.stop(); }
  });

test('writer output and failure responses are bounded and release admission', { timeout: 10000 }, async () => {
  const server = await startServer();
  try {
    for (const joint of ['right_worker_output', 'right_worker_failure']) {
      const response = await server.post({ payload: { ...valid, joint } });
      assert.equal(response.status, 502);
      assert.equal(response.body.ok, false);
      assert.ok(JSON.stringify(response.body).length < 256);
    }
    assert.equal((await server.post()).status, 200);
  } finally { await server.stop(); }
});

async function checkoutFixture() {
  const root = await mkdtemp(path.join(suite, 'checkout-'));
  for (const relative of [...fixtureFiles, 'tools/limit-sync-local/dist/server.js',
    'tools/limit-sync-local/package.json']) {
    const destination = path.join(root, relative);
    await mkdir(path.dirname(destination), { recursive: true });
    await copyFile(path.join(repoRoot, relative), destination);
  }
  return root;
}

async function snapshot(root) {
  return Promise.all(fixtureFiles.map(relative => readFile(path.join(root, relative), 'utf8')));
}

function scalar(block, key) {
  const value = block.match(new RegExp(`^\\s*${key}: ([^\\r\\n]+)`, 'm'))?.[1];
  assert.ok(value, `persisted ${key} must be present`);
  return Number(value);
}

async function assertMirrored(root, accepted) {
  const [, motors, control, , urdf] = await snapshot(root);
  const motor = motors.split(/\r?\n\s*- joint: /).find(block => block.startsWith(accepted.joint + '\n'));
  const entry = control.match(new RegExp(`^    ${accepted.joint}:\\r?\\n([\\s\\S]*?)(?=^    [a-z_]+:|(?![\\s\\S]))`, 'm'))?.[1];
  assert.ok(motor && entry, 'actual writer must retain the target joint');
  assert.equal(scalar(motor, 'position_lower_rad'), accepted.lower);
  assert.equal(scalar(motor, 'position_upper_rad'), accepted.upper);
  assert.equal(scalar(entry, 'position_soft_lower_rad'), accepted.soft_lower);
  assert.equal(scalar(entry, 'position_soft_upper_rad'), accepted.soft_upper);
  const joint = urdf.match(new RegExp(`<joint name="${accepted.joint}"[^>]*>([\\s\\S]*?)</joint>`))?.[1];
  const lower = Number(joint?.match(/<limit[^>]*lower="([^"]+)"/)?.[1]);
  const upper = Number(joint?.match(/<limit[^>]*upper="([^"]+)"/)?.[1]);
  assert.ok(lower <= accepted.lower && upper >= accepted.upper, 'URDF must cover accepted hard bounds');
}

test('actual Rust mirror writes the accepted negative bounds into only the supplied checkout',
  { timeout: 15000 }, async () => {
    const root = await checkoutFixture();
    const other = await checkoutFixture();
    const untouched = await snapshot(other);
    const source = await snapshot(repoRoot);
    const server = await startServer({ root, binary: realWriter, configOverride: path.join(other, 'config') });
    try {
      const response = await server.post();
      assert.equal(response.status, 200, JSON.stringify(response.body));
      await assertMirrored(root, valid);
      assert.deepEqual(await snapshot(other), untouched, 'runtime override must not redirect a local write');
      assert.deepEqual(await snapshot(repoRoot), source, 'actual source and physical configuration remain untouched');
    } finally { await server.stop(); }
  });

test('actual Rust mirror ignores competing runtime config for positive bounds too',
  { timeout: 15000 }, async () => {
    const root = await checkoutFixture();
    const other = await checkoutFixture();
    const untouched = await snapshot(other);
    const accepted = { ...valid, lower: 0.2, upper: 0.8, soft_lower: 0.25, soft_upper: 0.75 };
    const server = await startServer({ root, binary: realWriter, configOverride: path.join(other, 'config') });
    try {
      const response = await server.post({ payload: accepted });
      assert.equal(response.status, 200, JSON.stringify(response.body));
      await assertMirrored(root, accepted);
      assert.deepEqual(await snapshot(other), untouched);
    } finally { await server.stop(); }
  });

test('session work is limited even if callers rotate body metadata', { timeout: 15000 }, async () => {
  const server = await startServer();
  try {
    for (let index = 0; index < 30; index++) {
      assert.equal((await server.post({ payload: { ...valid, client_id: `fixture-${index}` } })).status, 200);
    }
    assert.equal((await server.post({ payload: { ...valid, client_id: 'another-fixture' } })).status, 429);
    assert.equal((await server.calls()).length, 30);
  } finally { await server.stop(); }
});

test('generated credentials rotate with the server session', { timeout: 10000 }, async () => {
  const first = await startServer({ generated: true });
  const second = await startServer({ generated: true });
  try {
    assert.ok(first.token.length >= 40);
    assert.notEqual(first.token, second.token);
    assert.equal((await second.post({ auth: first.token })).status, 401);
    assert.deepEqual(await second.calls(), []);
    assert.equal((await second.post()).status, 200);
  } finally { await first.stop(); await second.stop(); }
});
