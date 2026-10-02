from pathlib import Path
import hashlib, json, re, subprocess

backup = Path(r'J:\code\marengo-migration-backup-20260929')
source = 'b18faf486073b7311a029761792be171dac457ef'
root = backup/'batch32-mutants-b18faf4'
target = backup/'batch32-mutant-target-b18faf4'
target.mkdir(exist_ok=False)
frozen = json.loads((backup/'batch32-probe-freeze-b18faf4.json').read_text(encoding='utf-8'))
assert frozen['source'] == source

def verify_probes():
    for name, expected in frozen['git_blob_sha256'].items():
        assert hashlib.sha256((root/name).read_bytes()).hexdigest() == expected, name

records = []
def run(name, package, selected, expect_failure=False, mutation=None, binary=None):
    verify_probes()
    log = backup/f'batch32-mutation-{name}-b18faf4.txt'
    cmd = ['docker','compose','-p','marengo','run','--rm',
           '-v',f'{target.as_posix()}:/mutant-target',
           '-e','CARGO_TARGET_DIR=/mutant-target','dev','cargo','test','--locked','-p',package]
    cmd += ['--bin', binary] if binary else ['--lib']
    cmd += [selected,'--','--test-threads=1']
    if expect_failure:
        cmd += ['--exact']
    with log.open('wb') as output:
        completed = subprocess.run(cmd,cwd=root,stdout=output,stderr=subprocess.STDOUT)
    body = log.read_text(encoding='utf-8',errors='replace')
    outcomes = re.findall(r'test result: (ok|FAILED)\. (\d+) passed; (\d+) failed; (\d+) ignored;',body)
    behavioral = completed.returncode == 101 and f'test {selected} ... FAILED' in body and any(
        item[0]=='FAILED' and item[2]=='1' for item in outcomes) and 'panicked at' in body
    valid = behavioral if expect_failure else completed.returncode == 0 and any(item[0]=='ok' for item in outcomes)
    receipt = {'name':name,'source':source,'command':cmd,'exit':completed.returncode,
               'expected_behavioral_failure':expect_failure,'valid':valid,'outcomes':outcomes,
               'log':log.name,'log_sha256':hashlib.sha256(log.read_bytes()).hexdigest(),
               'mutation':mutation,'frozen_probe_files':len(frozen['git_blob_sha256'])}
    records.append(receipt)
    (backup/'batch32-mutation-receipt-b18faf4.json').write_text(
        json.dumps({'source':source,'runs':records},indent=2)+'\n',encoding='utf-8')
    print(f'{name}: exit={completed.returncode}, valid={valid}, results={outcomes}',flush=True)
    assert valid, f'{name}: compile/setup/missing-test failures are not qualifying reds; inspect {log}'

run('baseline-davout','davout','reference_')
run('baseline-berthier','berthier','reference_journal_tests')
run('baseline-pi','marengo-pi','reference_journal_shutdown_tests',binary='marengo-pi')

mutations = [
 ('cancel-lifecycle','crates/davout/src/reference_commit.rs',[
  ('        if entry.phase.get() == ReferenceCommitPhase::Pending {\n            entry.phase.set(ReferenceCommitPhase::Complete);\n        }',
   '        entry.phase.set(ReferenceCommitPhase::Complete);')],
  'davout','reference_journal_tests::late_real_write_does_not_rewrite_operator_cancellation_or_send_extra_stops'),
 ('deadline-equality','crates/davout/src/reference_transaction.rs',[
  ('if (backend.now)(&self.bus) >= stage.overall_deadline {','if (backend.now)(&self.bus) > stage.overall_deadline {')],
  'davout','reference_journal_tests::queued_real_completion_loses_eligibility_at_deadline_equality'),
 ('float-zero','crates/davout/src/reference_codec.rs',[
  ('Ok(Value::F32(value.to_bits()))','Ok(Value::F32(if value == 0.0 { 0 } else { value.to_bits() }))')],
  'davout','reference_codec::tests::scalar_tags_keep_exact_signed_zero_subnormal_and_integer_widths'),
 ('missing-sql-commit','crates/davout/src/reference_journal.rs',[
  ('        tx.commit()\n            .map_err(|cause| ReferenceJournalResult::Uncertain {\n                message: cause.to_string(),\n            })?;',
   '        std::mem::forget(tx);')],
  'davout','reference_journal_tests::controlled_child_death_before_and_after_real_commit_recovers_history_only'),
 ('credit-capacity','crates/davout/src/reference_journal.rs',[
  ('pub(super) const CREDIT_CAPACITY: usize = 8;','pub(super) const CREDIT_CAPACITY: usize = 1;')],
  'davout','reference_journal_tests::all_eight_accepted_credits_survive_cancellation_and_cache_eviction'),
 ('completion-before-fresh-report','crates/davout/src/reference_commit.rs',[
  ('        let joint = self.reference_commits.get(handle)?.stage.joint();',
   '        self.reference_commits.collect(handle);\n        let joint = self.reference_commits.get(handle)?.stage.joint();'),
  ('        self.observe_reference_commits();\n        self.reference_commits.collect(handle);\n        self.reference_commit_snapshot(handle)',
   '        self.observe_reference_commits();\n        self.reference_commit_snapshot(handle)')],
  'davout','reference_commit::tests::completed_unconsumed_write_still_loses_to_one_fresh_bounded_fault_report'),
 ('cancelled-work-scheduling','crates/berthier/src/loop.rs',[
  ('if self.supervisor.reference_work_pending() {','if self.supervisor.reference_busy() {')],
  'berthier','reference_journal_tests::cancelled_accepted_disk_work_still_advances_and_discards_new_unpermitted_intent'),
 ('wal-format-preflight','crates/davout/src/reference_journal.rs',[
  ('if &header[..16] != b"SQLite format 3\\0" || header[18..20] != [1, 1] {','if false {')],
  'davout','reference_journal::resource_tests::wal_format_with_corrupt_history_is_preserved_by_actual_worker'),
 ('sidecar-independence','crates/davout/src/reference_journal.rs',[
  ('for suffix in ["", "-journal", "-wal", "-shm"] {','for suffix in [""] {')],
  'davout','reference_journal::namespace_tests::preexisting_calibration_history_cannot_be_admitted_as_a_sqlite_sidecar'),
 ('counter-wrap','crates/davout/src/reference_commit.rs',[
  ('        let next = self\n            .reference_commits\n            .next_sequence\n            .checked_add(1)\n            .ok_or(ReferenceCommitError::CounterExhausted)?;',
   '        let next = self.reference_commits.next_sequence.wrapping_add(1);')],
  'davout','reference_commit::tests::exhausted_commit_identity_refuses_actual_stage_before_disk_admission'),
 ('hardlink-independence','crates/davout/src/reference_journal.rs',[
  ('match same_file::is_same_file(&history, &resource) {','match Ok::<bool, std::io::Error>(false) {')],
  'davout','reference_journal::hardlink_tests::existing_database_refuses_history_linked_to_its_rollback_resource'),
 ('regular-resource-guard','crates/davout/src/reference_journal.rs',[
  ('if history_is_regular && journal_is_regular {','if history_is_regular {')],
  'davout','reference_journal::filetype_tests::named_pipe_resources_never_block_identity_preflight_or_legacy_loading'),
]

for name, filename, replacements, package, selected in mutations:
    path = root/filename
    original = path.read_bytes()
    modified = original.decode('utf-8')
    for old, new in replacements:
        assert modified.count(old) == 1, (name,old,modified.count(old))
        modified = modified.replace(old,new)
    try:
        path.write_bytes(modified.encode('utf-8'))
        binding = {'path':filename,'before_sha256':hashlib.sha256(original).hexdigest(),
                   'after_sha256':hashlib.sha256(path.read_bytes()).hexdigest(),
                   'replacement_count':len(replacements)}
        run(name,package,selected,True,binding)
    finally:
        path.write_bytes(original)
        verify_probes()

run('unchanged-replay-davout','davout','reference_')
run('unchanged-replay-berthier','berthier','reference_journal_tests')
run('unchanged-replay-pi','marengo-pi','reference_journal_shutdown_tests',binary='marengo-pi')
manifest = json.loads((backup/'batch32-pi-source-b18faf4-manifest.json').read_text(encoding='utf-8'))
for filename, expected in manifest['file_sha256'].items():
    assert hashlib.sha256((root/filename).read_bytes()).hexdigest() == expected, filename
print(f'All twelve meaningful mutants failed; all ten frozen probes and {len(manifest["file_sha256"])} restored source files unchanged.',flush=True)
