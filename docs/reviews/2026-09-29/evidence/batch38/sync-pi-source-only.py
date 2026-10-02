from pathlib import Path
import datetime, hashlib, json, re, sqlite3, ssl, subprocess, sys, time, urllib.request

old, new, stage_name = sys.argv[1:]
assert re.fullmatch('[0-9a-f]{40}', old) and re.fullmatch('[0-9a-f]{40}', new)
assert re.fullmatch('batch38-[a-z0-9-]+', stage_name)
stage = Path('/home/joey/marengo-validation') / stage_name
assert stage.resolve() == stage and not stage.exists()
stage.mkdir(mode=0o700)
repo, installed = Path('/home/joey/marengo'), Path('/opt/marengo')
def git(*args):
    return subprocess.check_output(['git', '-C', str(repo), *args])
assert git('rev-parse', 'HEAD').decode().strip() == old
assert not git('status', '--porcelain').strip()
assert (installed / '.deploy-rev').read_text().startswith('2d0fd4088b0ca0301b2e27b07560b54b40ec61ca ')
subprocess.run(['git', '-C', str(repo), 'fetch', 'origin', 'main'], check=True, capture_output=True)
assert git('rev-parse', 'origin/main').decode().strip() == new
subprocess.run(['git', '-C', str(repo), 'merge-base', '--is-ancestor', old, new], check=True)
changes = git('diff', '--name-only', old, new).decode().splitlines()
assert all(name.startswith('docs/') for name in changes), changes
non_document_names = [name for name in git('ls-tree', '-r', '--name-only', new).decode().splitlines() if not name.startswith('docs/')]
assert len(non_document_names) == 1464
for name in non_document_names:
    assert git('rev-parse', '2d0fd4088b0ca0301b2e27b07560b54b40ec61ca:' + name) == git('rev-parse', new + ':' + name), name
bundle = stage / 'source-all-refs-before-sync.bundle'
subprocess.run(['git', '-C', str(repo), 'bundle', 'create', str(bundle), '--all'], check=True, capture_output=True)
verification = subprocess.run(['git', '-C', str(repo), 'bundle', 'verify', str(bundle)], check=True, capture_output=True)
(stage / 'source-all-refs-verification.txt').write_bytes(verification.stdout + verification.stderr)
hashfile = lambda p: hashlib.sha256(p.read_bytes()).hexdigest()
names = [installed / 'bin' / name for name in ('marengo-pi', 'motor-repl', 'marengo-gateway', 'marengo-log-cli', 'imu-probe')]
names += [installed / name for name in ('.deploy-rev', 'config/motors.yaml', 'config/control.yaml', 'config/robot.yaml', 'assets/urdf/marengo.urdf', 'var/calibration/zero_registry.yaml', 'www/index.html')]
names += [Path('/etc/marengo/env')]
before = {str(p): hashfile(p) for p in names}
staged_names = [p for parent in ('bin', 'www', 'consul/dist') for p in (repo / parent).rglob('*') if p.is_file() and not p.is_symlink()]
staged_before = {str(p): hashfile(p) for p in staged_names}
services_cmd = ['systemctl', 'show', 'marengo-pi.service', 'marengo-gateway.service', 'marengo-can.service', '-p', 'Id', '-p', 'MainPID', '-p', 'ActiveState', '-p', 'SubState', '-p', 'NRestarts']
services_before = subprocess.check_output(services_cmd, text=True)
subprocess.run(['git', '-C', str(repo), '-c', 'merge.autoStash=false', 'merge', '--ff-only', new], check=True, capture_output=True)
assert git('rev-parse', 'HEAD').decode().strip() == new
assert not git('status', '--porcelain').strip()
assert {str(p): hashfile(p) for p in names} == before
assert {str(p): hashfile(p) for p in staged_names} == staged_before
services_after = subprocess.check_output(services_cmd, text=True)
assert services_after == services_before
assert services_after.count('ActiveState=active') == 3 and services_after.count('NRestarts=0') == 3
texts = {}
for name, proto in [('safety', 'SafetyState'), ('state', 'RobotState')]:
    with urllib.request.urlopen('http://127.0.0.1:8080/snapshot/robot/' + name, timeout=5) as response:
        raw = response.read(1048577)
    assert len(raw) <= 1048576
    decoded = subprocess.run(['/usr/local/bin/protoc', '--proto_path=' + str(repo / 'proto'), '--decode=marengo.v1.' + proto, 'marengo/v1/marengo.proto'], input=raw, capture_output=True, check=True).stdout
    (stage / (name + '.pb')).write_bytes(raw)
    (stage / (name + '.txt')).write_bytes(decoded)
    texts[name] = decoded.decode()
    stamp = int(re.search(r'^timestamp_ms: (\d+)$', texts[name], re.M)[1])
    assert 0 <= int(time.time() * 1000) - stamp < 5000
assert 'mode: OPERATIONAL_MODE_DISABLED' in texts['safety']
assert 'JOINT_HOMING_STATE_VERIFIED' not in texts['state']
context = ssl.create_default_context(cafile=str(installed / 'var/gateway/tls/cert.pem'))
with urllib.request.urlopen('https://127.0.0.1:8444/', context=context, timeout=5) as response:
    index = response.read(2097153)
    assert response.status == 200 and index == (installed / 'www/index.html').read_bytes()
connection = sqlite3.connect('file:/opt/marengo/var/marengo.db?mode=ro', uri=True)
try:
    integrity = connection.execute('PRAGMA integrity_check').fetchone()[0]
    schema = json.loads(connection.execute("SELECT value_json FROM settings WHERE key='schema_version'").fetchone()[0])
    assert integrity == 'ok' and schema == 3
finally:
    connection.close()
links = json.loads(subprocess.check_output(['ip', '-json', '-statistics', 'link', 'show', 'can0']))
receipt = {
    'recorded_UTC': datetime.datetime.now(datetime.timezone.utc).isoformat(),
    'source_before': old, 'source_after': new, 'source_clean': True,
    'changed_document_files': len(changes), 'qualified_non_document_inputs_verified': len(non_document_names),
    'installed_release': (installed / '.deploy-rev').read_text().strip(),
    'installed_bin_policy_model_calibration_environment_hashes_unchanged': True,
    'staged_payload_files_preserved': len(staged_names), 'services_and_PIDs_unchanged': True,
    'services': services_after, 'installed_hashes': before,
    'source_all_refs_backup_verified_SHA256': hashfile(bundle),
    'Store_integrity': integrity, 'Store_schema': schema, 'trusted_HTTPS': 200,
    'mode': 'Disabled', 'unhomed_joints': texts['state'].count('JOINT_HOMING_STATE_UNHOMED'),
    'faulted_joints': texts['state'].count('JOINT_HOMING_STATE_FAULTED'),
    'software_latch': 'software_estop_latched: true' in texts['safety'],
    'CAN0_rx_errors': links[0]['stats64']['rx']['errors'], 'physical_acceptance': False,
    'scope': 'Source-only fast-forward after verified backup and installed/staged hash checks. No installer, service restart or motor command; installed runtime remains qualified2d0fd40.'
}
(stage / 'source-sync-receipt.json').write_text(json.dumps(receipt, indent=2) + '\n')
print(json.dumps(receipt, indent=2))
