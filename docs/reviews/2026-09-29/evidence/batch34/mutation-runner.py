from pathlib import Path
import hashlib
import json
import re
import subprocess

work = Path(__file__).parent
source = '9f1e52306f616d844c313b0a2dec3d7a6c0733e6'
root = work / 'mutants-9f1e523'
target = work / 'mutant-target-9f1e523'
target.mkdir(exist_ok=False)
frozen = json.loads((work / 'probe-freeze-9f1e523.json').read_text(encoding='utf-8'))
manifest = json.loads((work / 'pi-source-9f1e523-manifest.json').read_text(encoding='utf-8'))
runs = []

def verify_probes():
    for name, expected in frozen['git_blob_sha256'].items():
        assert hashlib.sha256((root / name).read_bytes()).hexdigest() == expected, name

def save(restored=False):
    receipt = {'source': source, 'frozen_probes': len(frozen['git_blob_sha256']), 'runs': runs}
    if restored:
        receipt['all_source_and_probe_files_restored'] = len(manifest['file_sha256'])
    (work / 'mutation-receipt-9f1e523.json').write_text(json.dumps(receipt, indent=2) + '\n', encoding='utf-8')

def run(name, package, selected, failure=False, binding=None, binary=None):
    verify_probes()
    log = work / f'mutation-{name}-9f1e523.txt'
    command = ['docker', 'compose', '-p', 'marengo', 'run', '--rm', '-T', '--no-deps',
               '-v', f'{target.as_posix()}:/mutant-target', '-e', 'CARGO_TARGET_DIR=/mutant-target',
               '-e', 'CARGO_BUILD_JOBS=2', 'dev', 'cargo', 'test', '--locked', '-p', package]
    command += ['--bin', binary] if binary else ['--lib']
    command += [selected, '--', '--test-threads=1']
    if failure:
        command += ['--exact']
    with log.open('wb') as output:
        result = subprocess.run(command, cwd=root, stdout=output, stderr=subprocess.STDOUT)
    body = log.read_text(encoding='utf-8', errors='replace')
    outcomes = re.findall(r'test result: (ok|FAILED)\. (\d+) passed; (\d+) failed; (\d+) ignored;', body)
    named_run = re.search(r'(?m)^test ' + re.escape(selected) + r' \.\.\.', body) is not None
    named_failure = re.search(r'(?m)^\s+' + re.escape(selected) + r'$', body) is not None
    named_panic = f'---- {selected} stdout ----' in body and 'panicked at' in body
    behavioral = result.returncode == 101 and named_run and named_failure and named_panic and outcomes[-1:] == [('FAILED', '0', '1', '0')]
    valid = behavioral if failure else result.returncode == 0 and bool(outcomes) and all(item[0] == 'ok' for item in outcomes)
    executable = re.search(r'Running unittests src/(?:lib|main)\.rs \((/mutant-target/[^)]+)\)', body)
    executable_receipt = None
    if executable:
        relative = executable.group(1).removeprefix('/mutant-target/')
        assert '..' not in Path(relative).parts
        path = target / relative
        executable_receipt = {'path': relative, 'sha256': hashlib.sha256(path.read_bytes()).hexdigest()}
    runs.append({'name': name, 'source': source, 'command': command, 'exit_code': result.returncode,
                 'expected_behavioral_failure': failure, 'valid': valid, 'outcomes': outcomes,
                 'log': log.name, 'log_sha256': hashlib.sha256(log.read_bytes()).hexdigest(),
                 'mutation': binding, 'test_executable': executable_receipt,
                 'frozen_probe_files': len(frozen['git_blob_sha256'])})
    save()
    print(f'{name}: exit={result.returncode}, valid={valid}, results={outcomes}', flush=True)
    assert valid, f'{name}: compiler/setup/missing-test failures are not qualifying reds; inspect {log}'

for name, expected in manifest['file_sha256'].items():
    assert hashlib.sha256((root / name).read_bytes()).hexdigest() == expected, name
run('baseline-davout', 'davout', 'current_grant_')
run('baseline-berthier', 'berthier', 'current_grant_')
run('baseline-initial-shutdown', 'marengo-pi', 'shutdown_tests::active_shutdown_clears_gain_torque_and_wave_intent_before_storage_and_reenable', binary='marengo-pi')

davout = 'reference_commit::grant_tests::'
mutations = [
    ('controller-active-shortcut', 'crates/berthier/src/loop.rs', [
        ('        let targets = if self.supervisor.mode() == OperationalMode::Active {\n            self.supervisor.active_joints().iter().cloned().collect()\n        } else {\n            self.supervisor.resolve_enable_targets(&self.repo_root)?\n        };',
         '        if self.supervisor.mode() == OperationalMode::Active {\n            return Ok(());\n        }\n        let targets = self.supervisor.resolve_enable_targets(&self.repo_root)?;')],
     'berthier', 'reference_grant_tests::current_grant_controller_active_motion_entry_rechecks_revoked_permission', None),
    ('unconditional-initial-shutdown', 'crates/davout/src/reference_transaction.rs', [
        ('        if self.reference_authority.consumed_binding().is_some() {\n            self.reference_authority.revoke();\n        }\n        self.invalidate_retained_stages(ReferenceStageInvalidation::Shutdown);',
         '        self.reference_authority.revoke();\n        self.invalidate_retained_stages(ReferenceStageInvalidation::Shutdown);')],
     'marengo-pi', 'shutdown_tests::active_shutdown_clears_gain_torque_and_wave_intent_before_storage_and_reenable', 'marengo-pi'),
    ('selected-peer-permission', 'crates/davout/src/reference.rs', [
        ('joints: HashSet::from([joint.to_owned()]),', 'joints: policy.motors.motors.iter().map(|motor| motor.joint.clone()).collect(),')],
     'davout', davout + 'current_grant_requires_real_durable_fresh_consumption_and_covers_only_its_joint', None),
    ('ignore-selected-device-reset', 'crates/davout/src/lib.rs', [
        ('!= Some(binding.device_epoch)', '!= Some(binding.device_epoch) && false')],
     'davout', davout + 'current_grant_device_reset_and_owning_commit_cancel_revoke_exact_selection', None),
    ('deadline-equality', 'crates/davout/src/reference_transaction.rs', [
        ('if (backend.now)(&self.bus) >= stage.overall_deadline {', 'if (backend.now)(&self.bus) > stage.overall_deadline {')],
     'davout', davout + 'current_grant_completed_unconsumed_history_loses_at_original_deadline_equality', None),
    ('missing-current-counter-preflight', 'crates/davout/src/reference_commit.rs', [
        ('if self.reference_commits.selection == CommitSelection::CurrentVirtual\n            && self\n                .reference_authority\n                .generation()\n                .checked_add(1)\n                .is_none()',
         'if false')],
     'davout', davout + 'current_grant_counter_exhaustion_refuses_commit_before_acceptance_or_permission_change', None),
    ('disable-revokes-selected', 'crates/davout/src/reference_commit.rs', [
        ('if reason == ReferenceCancelReason::Shutdown && self.reference_commits.journal.is_some() {',
         'if matches!(reason, ReferenceCancelReason::Shutdown | ReferenceCancelReason::Disable) && self.reference_commits.journal.is_some() {')],
     'davout', davout + 'current_grant_lifetime_survives_old_deadline_and_successful_ordinary_disable', None),
    ('cancelled-durable-grant', 'crates/davout/src/reference_commit.rs', [
        ('                && entry.phase.get() == ReferenceCommitPhase::Complete\n', ''),
        ('                && self.retained_stage_status(&entry.stage)\n                    == ReferenceStageStatus::CurrentVirtualEvidence\n', '')],
     'davout', davout + 'current_grant_cancel_and_shutdown_win_before_late_durable_publication', None),
]

# Deliberately select and return before the mandatory fresh receive report.
commit = (root / 'crates/davout/src/reference_commit.rs').read_text(encoding='utf-8')
start = commit.index('        if let Some(entry) = self.reference_commits.collect(handle) {')
end = commit.index('        self.reference_commit_snapshot(handle)\n    }', start)
selection = commit[start:end]
mutations.append(('completion-before-fresh-report', 'crates/davout/src/reference_commit.rs', [
    ('        let joint = self.reference_commits.get(handle)?.stage.joint();',
     '        if self.reference_commits.selection == CommitSelection::CurrentVirtual {\n' + selection +
     '            return self.reference_commit_snapshot(handle);\n        }\n        let joint = self.reference_commits.get(handle)?.stage.joint();')],
    'davout', davout + 'current_grant_completed_unconsumed_history_loses_to_fresh_fault_or_work_limit', None))

for name, filename, replacements, package, selected, binary in mutations:
    path = root / filename
    original = path.read_bytes()
    modified = original.decode('utf-8')
    for old, new in replacements:
        assert modified.count(old) == 1, (name, old, modified.count(old))
        modified = modified.replace(old, new)
    try:
        path.write_bytes(modified.encode('utf-8'))
        binding = {'path': filename, 'before_sha256': hashlib.sha256(original).hexdigest(),
                   'after_sha256': hashlib.sha256(path.read_bytes()).hexdigest(), 'replacement_count': len(replacements)}
        if name == 'controller-active-shortcut':
            old = subprocess.check_output(['git', 'show', 'd658527f0d5bca89c3d9d5b5848946a297058e73:' + filename], cwd=work / 'source-9f1e523')
            assert path.read_bytes() == old, 'Active mutation must be exact earlier production file'
            binding['exact_earlier_file_commit'] = 'd658527f0d5bca89c3d9d5b5848946a297058e73'
        run(name, package, selected, True, binding, binary)
    finally:
        path.write_bytes(original)
        verify_probes()

run('unchanged-replay-davout', 'davout', 'current_grant_')
run('unchanged-replay-berthier', 'berthier', 'current_grant_')
run('unchanged-replay-initial-shutdown', 'marengo-pi', 'shutdown_tests::active_shutdown_clears_gain_torque_and_wave_intent_before_storage_and_reenable', binary='marengo-pi')
for name, expected in manifest['file_sha256'].items():
    assert hashlib.sha256((root / name).read_bytes()).hexdigest() == expected, name
save(restored=True)
print(f'All9 production mutants rejected; {len(manifest["file_sha256"])} files restored; final17/3/1 probes replayed unchanged.', flush=True)
