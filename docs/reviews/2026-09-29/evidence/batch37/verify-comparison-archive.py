from pathlib import Path
import argparse,hashlib,json,subprocess,sys,tarfile
p=argparse.ArgumentParser();p.add_argument('phase',choices=('B1','A2','B2','activation'));a=p.parse_args()
r=Path(__file__).resolve().parent
archive=r/('batch37-'+a.phase+'-observation.tar.gz')
out=r/(a.phase+'-native' if a.phase!='activation' else 'candidate-activation')
out.mkdir(exist_ok=True)
with tarfile.open(archive) as t:
 members=t.getmembers()
 for m in members:assert m.isfile() and not Path(m.name).is_absolute() and len(Path(m.name).parts)==1 and m.size<=24*1024*1024,m.name
 manifest=json.loads(t.extractfile('observation-manifest.json').read())
 assert len(members)==len(manifest)+1
 for m in members:
  raw=t.extractfile(m).read()
  if m.name!='observation-manifest.json':assert hashlib.sha256(raw).hexdigest()==manifest[m.name],m.name
 # All bytes verified in the retained archive; extract only named evidence.
 names={m.name for m in members if not m.name.startswith('sample-')}
 if a.phase!='activation':
  receipt=json.loads(t.extractfile('diagnostic-receipt.json').read())
  last=len(receipt['snapshot_samples'])-1
  for index in (0,last):
   for kind in ('safety','state'):
    for ext in ('pb','txt'):names.add(f'sample-{index:02d}-{kind}.{ext}')
 for n in names:
  raw=t.extractfile(n).read();dest=out/n
  if dest.exists():assert dest.read_bytes()==raw,n
  else:dest.write_bytes(raw)
 result={'phase':a.phase,'archive_SHA256':hashlib.sha256(archive.read_bytes()).hexdigest(),'all_archived_files_verified':len(members),'manifest_SHA256':hashlib.sha256(t.extractfile('observation-manifest.json').read()).hexdigest()}
(r/(a.phase+'-archive-verification.json')).write_text(json.dumps(result,indent=2)+'\n')
if a.phase!='activation':
 c=subprocess.run([sys.executable,str(r/'overflow-oracle.py'),str(out)],capture_output=True)
 for suffix,data in (('json',c.stdout),('stderr',c.stderr),('exit-code',(str(c.returncode)+'\n').encode())):
  dest=r/(a.phase+'-oracle.'+suffix)
  if dest.exists():
   if suffix=='exit-code':assert int(dest.read_text())==c.returncode,dest
   else:assert dest.read_bytes()==data,dest
  else:dest.write_bytes(data)
 assert c.returncode in (0,42),c.stderr.decode()
 result['frozen_oracle_exit']=c.returncode
print(json.dumps(result))
