// Build with known disposable credential inputs and refuse an exposed asset.
import { randomBytes, createHash } from 'node:crypto';
import { spawnSync } from 'node:child_process';
import { lstatSync, readdirSync, readFileSync } from 'node:fs';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

export function qualifyAssets(directory, markers) {
  const files = [];
  const visit = (path) => {
    const stat = lstatSync(path);
    if (stat.isSymbolicLink()) throw new Error('Credential qualification refuses redirected asset paths');
    if (stat.isDirectory()) {
      for (const name of readdirSync(path).sort()) visit(join(path, name));
    } else if (stat.isFile()) {
      const bytes = readFileSync(path);
      if (markers.some((marker) => bytes.includes(Buffer.from(marker)))) {
        throw new Error('A known runtime credential fixture was found in a built asset');
      }
      files.push({ path: path.slice(directory.length + 1).replaceAll('\\', '/'), sha256: createHash('sha256').update(bytes).digest('hex') });
    }
  };
  visit(directory);
  if (!files.some((file) => file.path === 'index.html')) throw new Error('Built index.html missing');
  return files;
}

if (process.argv[1] && resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  const consul = resolve(dirname(fileURLToPath(import.meta.url)), '..');
  if (!process.env.npm_execpath) throw new Error('Run with npm run build:qualified');
  const names = [
    'VITE_MARENGO_LOG_TOKEN', 'VITE_AUTO_LEARN_TOKEN', 'LIMIT_SYNC_TOKEN', 'AUTO_LEARN_TOKEN',
    ...['LOG', 'OPERATOR', 'CONTROL', 'CALIBRATION', 'CONFIG', 'MANAGEMENT', 'READ'].map((role) => `MARENGO_GATEWAY_${role}_TOKEN`),
  ];
  const fixtures = Object.fromEntries(names.map((name) => [name, `owned-credential-fixture-${randomBytes(24).toString('hex')}`]));
  const build = spawnSync(process.execPath, [process.env.npm_execpath, 'run', 'build', ...process.argv.slice(2)], {
    cwd: consul, env: { ...process.env, ...fixtures }, stdio: 'inherit', timeout: 300000,
  });
  if (build.error || build.status !== 0) throw new Error('Credential fixture build failed or timed out');
  const files = qualifyAssets(join(consul, 'dist'), Object.values(fixtures));
  const manifestSha256 = createHash('sha256').update(JSON.stringify(files)).digest('hex');
  process.stdout.write(`Runtime credential build: ${files.length} assets exclude ${names.length} supplied fixtures; manifest ${manifestSha256}\n`);
}
