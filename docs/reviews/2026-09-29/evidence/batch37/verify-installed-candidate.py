from pathlib import Path
import hashlib
import json
import sqlite3
import subprocess
import yaml

stage = Path('/home/joey/marengo-validation/pi-sync-20261002-batch37-comparison')
installed = Path('/opt/marengo')
preview = json.loads((stage / 'limit-preview-receipt.json').read_text())
manifest = (stage / 'release-candidate/bundle-sha256.txt').read_text().splitlines()
digest = lambda p: hashlib.sha256(p.read_bytes()).hexdigest()
checked = []
preserved = []
for line in manifest:
    expected, relative = line.split('  ', 1)
    if relative.startswith('target/release/'):
        destination = installed / 'bin' / relative.removeprefix('target/release/')
    else:
        destination = installed / relative
    assert destination.is_file() and not destination.is_symlink(), destination
    if relative in ('config/motors.yaml', 'config/control.yaml', 'assets/urdf/marengo.urdf'):
        expected = preview['preview_file_sha256'][relative]
        preserved.append(relative)
    assert digest(destination) == expected, (relative, digest(destination), expected)
    checked.append(relative)

calibration = installed / 'var/calibration/zero_registry.yaml'
assert digest(calibration) == digest(stage / 'backup/var/calibration/zero_registry.yaml')
env = Path('/etc/marengo/env')
assert digest(env) == digest(stage / 'backup/runtime.env'), 'Runtime environment changed'
old_motors = {m['joint']: m for m in yaml.safe_load((stage / 'backup/config/motors.yaml').read_text())['motors']}
current_motors = {m['joint']: m for m in yaml.safe_load((installed / 'config/motors.yaml').read_text())['motors']}
assert set(old_motors) == set(current_motors) and len(current_motors) == 5
identity_fields = ('driver', 'motor_type', 'can_interface', 'device_id', 'direction', 'gear_ratio', 'recv_can_id', 'firmware_version')
identity_changes = {joint: {key: [old_motors[joint].get(key), motor.get(key)] for key in identity_fields if old_motors[joint].get(key) != motor.get(key)} for joint, motor in current_motors.items()}
assert not any(identity_changes.values()), identity_changes
connection = sqlite3.connect(f'file:{installed}/var/marengo.db?mode=ro', uri=True)
try:
    integrity = connection.execute('PRAGMA integrity_check').fetchone()[0]
    assert integrity == 'ok', integrity
    schema_columns = [r[1] for r in connection.execute('PRAGMA table_info(settings)')]
    assert schema_columns == ['key', 'value_json', 'updated_ms'], schema_columns
    marker = connection.execute("SELECT value_json FROM settings WHERE key='schema_version'").fetchone()[0]
    assert json.loads(marker) == 3, marker
    tables = [r[0] for r in connection.execute("SELECT name FROM sqlite_master WHERE type='table' ORDER BY name")]
finally:
    connection.close()

for name in ('SafetyState', 'RobotState'):
    source = stage / ('post-safety.pb' if name == 'SafetyState' else 'post-state.pb')
    result = subprocess.run([
        '/usr/local/bin/protoc', '--proto_path=/home/joey/marengo/proto',
        f'--decode=marengo.v1.{name}', 'marengo/v1/marengo.proto'
    ], input=source.read_bytes(), capture_output=True, check=True)
    (stage / f'post-{name}.txt').write_bytes(result.stdout)

receipt = {
    'installed_revision': (installed / '.deploy-rev').read_text().strip(),
    'manifest_entries_verified': len(checked),
    'preserved_taught_files_verified_against_independent_preview': preserved,
    'all_five_joint_limits_and_motor_identity_preserved': preview['all_five_taught_hard_and_soft_values_preserved'] and not any(r['motor_identity_changes'] for r in preview['joint_envelopes']),
    'joint_envelopes': preview['joint_envelopes'],
    'actual_installed_motor_identity_fields_verified': identity_fields,
    'actual_installed_motor_identity_changes': identity_changes,
    'binary_sha256': {p.name: digest(installed / 'bin' / p.name) for p in (stage / 'release-candidate/target/release').iterdir() if p.is_file()},
    'calibration_registry_unchanged_sha256': digest(calibration),
    'runtime_environment_unchanged': True,
    'Store_integrity_check': integrity,
    'Store_schema_version': json.loads(marker),
    'Store_tables': tables,
    'operator_setup': 'Human explicitly confirmed stable/supported resting pose, clear workspace and physical E-stop within reach; motor power remains on.',
    'activation_exit_code': int((stage / 'install-candidate.exit-code').read_text()),
    'activation_started_UTC': (stage / 'activation-started.txt').read_text().strip(),
    'physical_motion': 'No enable, SetZero, target or motion test commanded. Live reference still unqualified.',
}
assert receipt['activation_exit_code'] == 0
assert receipt['all_five_joint_limits_and_motor_identity_preserved']
(stage / 'post-install-file-verification.json').write_text(json.dumps(receipt, indent=2) + '\n')
print(json.dumps({k: receipt[k] for k in ("installed_revision", "manifest_entries_verified", "all_five_joint_limits_and_motor_identity_preserved", "runtime_environment_unchanged", "Store_integrity_check", "Store_schema_version", "activation_exit_code", "physical_motion")}, indent=2))
