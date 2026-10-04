from pathlib import Path
import collections, hashlib, json, re, subprocess, sys

repo = Path('J:/code/marengo-worktrees/current-virtual-reference')
base = 'ba0fff7206877943fbca0f79b18d59089d5d4a24'
head = sys.argv[1] if len(sys.argv) > 1 else None
prefix = 'docs/reviews/2026-09-29/'
def blob(name, revision=head):
    spec = revision + ':' + name if revision else ':' + name
    return subprocess.check_output(['git', 'show', spec], cwd=repo)
def obj(name, revision=head):
    return json.loads(blob(prefix + name, revision).decode('utf-8-sig'))
old, new = obj('implementation-ledger.json', base), obj('implementation-ledger.json')
for key in ['current_batch','findings','architecture_and_maintenance_tasks','active_mac_continuation','finding_status_counts','automation_status']:
    assert new[key] == old[key], key
assert new['batch_history'][:-1] == old['batch_history']
assert new['batch_history'][-1]['batch_id'] == 'batch38'
assert len(new['findings']) == 102 and len(new['architecture_and_maintenance_tasks']) == 8
assert collections.Counter(f['status'] for f in new['findings']) == {'verified':26,'partial':13,'open':63}
assert blob(prefix + 'HANDOFF.md').endswith(blob(prefix + 'HANDOFF.md', base))
assert blob(prefix + 'implementation-roadmap.md').endswith(blob(prefix + 'implementation-roadmap.md', base))
command = ['git','diff','--name-only',base,head] if head else ['git','diff','--cached','--name-only',base]
changes = subprocess.check_output(command,cwd=repo,text=True).splitlines()
assert changes and all(name.startswith(prefix) for name in changes)
bindings = obj('evidence/batch38/artifact-bindings.json')
for binding in bindings['files']:
    raw = blob(prefix + 'evidence/batch38/' + binding['name'])
    assert len(raw) == binding['bytes'], binding['name']
    assert hashlib.sha256(raw).hexdigest() == binding['committed_SHA256'], binding['name']
    if binding['normalization'].startswith('Lossless JSON'):
        restored = json.loads(raw)['raw_UTF8'].encode('utf-8')
        assert hashlib.sha256(restored).hexdigest() == binding['original_SHA256'], binding['name']
for name, entry in obj('evidence/batch38/red-green-logs.json').items():
    restored = entry['raw_UTF8'].encode('utf-8')
    assert len(restored) == entry['bytes'] and hashlib.sha256(restored).hexdigest() == entry['SHA256'], name
assert hashlib.sha256(blob(prefix + 'evidence/batch38/frozen-receive-diagnostics-tests.rs')).hexdigest() == '139022845c7803f55051f93cac44ad2fd34ec3e169da41cb768bb917db6cd839'
for name in changes:
    raw = blob(name)
    if name.endswith('.json'):
        json.loads(raw.decode('utf-8-sig'))
for name in ['RIGHT-ARM-VALIDATION-HANDOFF.md','batch38-receive-diagnostics.md','repair-status-snapshot.md']:
    for target in re.findall(r'\]\(([^)]+)\)', blob(prefix+name).decode()):
        if '://' in target or target.startswith('#'):
            continue
        assert (repo / prefix / target.split('#')[0]).is_file(), (name,target)
for name, expected in [('source-ci.json','2d0fd4088b0ca0301b2e27b07560b54b40ec61ca'),('main-ci.json',base)]:
    ci = obj('evidence/batch38/'+name)
    assert ci['headSha'] == expected and ci['status'] == 'completed' and ci['conclusion'] == 'success'
    assert len(ci['jobs']) == 5 and all(job['conclusion']=='success' for job in ci['jobs'])
    assert any(job['name']=='sim' and any(step['name']=='Run check-sim.sh' and step['conclusion']=='success' for step in job['steps']) for job in ci['jobs'])
print(json.dumps({'head':head or 'staged','changed_review_files':len(changes),'artifact_bindings_verified':len(bindings['files']),'lossless_regression_logs_verified':3,'all_finding_and_maintenance_objects_preserved':True,'historical_HANDOFF_and_roadmap_suffixes_preserved':True,'source_and_main_all_five_actual_CI_verified':True,'new_handoff_relative_links_valid':True,'physical_acceptance':False}))
