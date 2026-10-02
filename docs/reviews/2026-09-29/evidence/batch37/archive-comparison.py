from pathlib import Path
import argparse,hashlib,json,os,tarfile
os.umask(0o077)
p=argparse.ArgumentParser();p.add_argument('--phase',required=True,choices=('B1','A2','B2','activation'));a=p.parse_args()
base=Path('/home/joey/marengo-validation')
s=base/('pi-sync-20261002-batch37-comparison' if a.phase=='activation' else 'batch37-can-comparison-20261002-'+a.phase)
assert s.resolve()==s
if a.phase=='activation':
 names=['activation-started.txt','activation-receipt.json','activation-traffic-analysis.json','activation-before-can0.txt','activation-before-can1.txt','activation-before-safety.pb','activation-before-state.pb','activation-before-safety.txt','activation-before-state.txt','activation-passive-can.log','activation-passive-can.stderr','install-candidate.txt','install-candidate.exit-code','post-safety.pb','post-state.pb','post-SafetyState.txt','post-RobotState.txt','post-can0.txt','post-can1.txt','post-services.txt','post-journal.txt','post-install-file-verification.json','inter-window-journal.txt','comparison-package-identity.json','switch-A2.txt','switch-A2.exit-code','switch-A2-receipt.json','switch-B2.txt','switch-B2.exit-code','switch-B2-receipt.json','limit-preview-receipt.json','release-staging-receipt.json','release-bundle-receipt.json','update-preparation.json','source-owned-payload-backup-manifest.json']
 files=[s/n for n in names]
else:
 receipt=json.loads((s/'diagnostic-receipt.json').read_text())
 assert receipt['restart_exit']==0 and receipt['passive_capture_exit'] in (0,124)
 assert hashlib.sha256((s/'passive-can.log').read_bytes()).hexdigest()==receipt['capture_SHA256']
 files=sorted(f for f in s.rglob('*') if f.is_file() and f.name!='observation-manifest.json')
for f in files:assert f.is_file() and not f.is_symlink() and f.resolve().is_relative_to(s),f
manifest={str(f.relative_to(s)):hashlib.sha256(f.read_bytes()).hexdigest() for f in files}
manifest_path=s/'observation-manifest.json'
with manifest_path.open('x') as f:f.write(json.dumps(manifest,indent=2)+'\n')
archive=base/('batch37-'+a.phase+'-observation.tar.gz')
assert not archive.exists()
with tarfile.open(archive,'w:gz') as t:
 for f in files+[manifest_path]:t.add(f,arcname=str(f.relative_to(s)),recursive=False)
with tarfile.open(archive) as t:
 assert len(t.getmembers())==len(manifest)+1
 for m in t.getmembers():
  if m.name=='observation-manifest.json':continue
  assert hashlib.sha256(t.extractfile(m).read()).hexdigest()==manifest[m.name],m.name
print(json.dumps({'phase':a.phase,'archive':str(archive),'archive_SHA256':hashlib.sha256(archive.read_bytes()).hexdigest(),'archived_files':len(manifest)+1,'all_observation_file_hashes_verified':True}))
