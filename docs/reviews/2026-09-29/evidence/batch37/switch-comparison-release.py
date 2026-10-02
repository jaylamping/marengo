from pathlib import Path
import argparse, datetime, hashlib, json, os, shutil, subprocess, tarfile, time, urllib.request
os.umask(0o077)
p=argparse.ArgumentParser()
p.add_argument('--label',required=True,choices=('A2','B2'))
a=p.parse_args()
s=Path('/home/joey/marengo-validation/pi-sync-20261002-batch37-comparison')
r=Path('/home/joey/marengo')
i=Path('/opt/marengo')
old='01a5c403bd8e790cd7403565c777cca8729e48c6'
new='0a3fc6bb0dd7ab652d97fd2efb7bb46ceee517c4'
wanted=old if a.label=='A2' else new
previous=new if a.label=='A2' else old
payload=Path('/home/joey/marengo-validation/pi-sync-20261002-batch35/release-main') if a.label=='A2' else s/'release-candidate'
current_payload=s/'release-candidate' if a.label=='A2' else Path('/home/joey/marengo-validation/pi-sync-20261002-batch35/release-main')
digest=lambda p:hashlib.sha256(p.read_bytes()).hexdigest()
assert r.resolve()==r and s.resolve()==s and payload.resolve()==payload
assert not (s/('switch-'+a.label+'.txt')).exists(), 'Never repeat an attempted install'
assert (i/'.deploy-rev').read_text().startswith(previous+' ')
assert subprocess.check_output(['git','-C',str(r),'rev-parse','HEAD'],text=True).strip()==previous
assert not subprocess.check_output(['git','-C',str(r),'status','--porcelain']).strip()
preview=json.loads((s/'limit-preview-receipt.json').read_text())
identity=json.loads((s/'comparison-package-identity.json').read_text())
expected_manifest=identity['known_package_manifest_SHA256'] if a.label=='A2' else json.loads((s/'release-bundle-receipt.json').read_text())['manifest_SHA256']
assert digest(payload/'bundle-sha256.txt')==expected_manifest
manifest=[line.split('  ',1) for line in (payload/'bundle-sha256.txt').read_text().splitlines()]
assert len(manifest)==214
for h,n in manifest:
 assert digest(payload/n)==h,n
for line in (current_payload/'bundle-sha256.txt').read_text().splitlines():
 h,n=line.split('  ',1)
 dest=i/'bin'/n.removeprefix('target/release/') if n.startswith('target/release/') else i/n
 if n in preview['preview_file_sha256']:h=preview['preview_file_sha256'][n]
 assert digest(dest)==h,(n,'currently installed hash mismatch')
for n in ('var/calibration/zero_registry.yaml',):assert digest(i/n)==digest(s/'backup'/n)
assert digest(Path('/etc/marengo/env'))==digest(s/'backup/runtime.env')
with urllib.request.urlopen('http://127.0.0.1:8080/snapshot/robot/safety',timeout=5) as f:raw=f.read(1048577)
assert len(raw)<=1048576
text=subprocess.run(['/usr/local/bin/protoc','--proto_path='+str(r/'proto'),'--decode=marengo.v1.SafetyState','marengo/v1/marengo.proto'],input=raw,capture_output=True,check=True).stdout
assert b'mode: OPERATIONAL_MODE_DISABLED' in text
# Preserve the source-owned release inputs before replacing only those inputs.
backup=s/'backup'/('source-owned-before-'+a.label+'.tar.gz')
assert not backup.exists()
with tarfile.open(backup,'w:gz') as archive:
 for h,n in [line.split('  ',1) for line in (current_payload/'bundle-sha256.txt').read_text().splitlines()]:
  assert digest(r/n)==h,(n,'source payload changed before switch')
  archive.add(r/n,arcname=n,recursive=False)
subprocess.run(['git','-C',str(r),'checkout','--detach',wanted],check=True,capture_output=True)
for h,n in manifest:
 dest=r/n
 assert dest.resolve()==dest and not dest.is_symlink()
 dest.parent.mkdir(parents=True,exist_ok=True)
 shutil.copyfile(payload/n,dest)
entries=subprocess.check_output(['git','-C',str(r),'ls-files','-s','-z','scripts']).split(b'\0')
for e in entries:
 if not e:continue
 meta,name=e.split(b'\t',1)
 mode=meta.split()[0]
 assert mode in (b'100644',b'100755')
 (r/os.fsdecode(name)).chmod(0o755 if mode==b'100755' else 0o644)
for dest in (r/'www',r/'consul/dist'):
 assert dest.resolve()==dest and dest.is_dir()
 subprocess.run(['rsync','-a','--delete',str(payload/'www')+'/',str(dest)+'/'],check=True,capture_output=True)
for h,n in manifest:assert digest(r/n)==h,n
assert not subprocess.check_output(['git','-C',str(r),'status','--porcelain']).strip()
started=datetime.datetime.now(datetime.timezone.utc).isoformat()
with (s/('switch-'+a.label+'.txt')).open('wb') as output:
 result=subprocess.run(['sudo','-n',str(r/'scripts/install-pi.sh')],cwd=r,stdout=output,stderr=subprocess.STDOUT,timeout=120)
(s/('switch-'+a.label+'.exit-code')).write_text(str(result.returncode)+'\n')
assert result.returncode==0,'Preserve failed installer; never repeat automatically'
for h,n in manifest:
 dest=i/'bin'/n.removeprefix('target/release/') if n.startswith('target/release/') else i/n
 if n in preview['preview_file_sha256']:h=preview['preview_file_sha256'][n]
 assert digest(dest)==h,(n,'actual installed verification failed')
assert digest(i/'var/calibration/zero_registry.yaml')==digest(s/'backup/var/calibration/zero_registry.yaml')
assert digest(Path('/etc/marengo/env'))==digest(s/'backup/runtime.env')
# A successful installer is never repeated for an HTTP readiness failure.
deadline=time.monotonic()+15
while True:
 try:
  with urllib.request.urlopen('http://127.0.0.1:8080/snapshot/robot/safety',timeout=3) as f:raw=f.read(1048577)
  break
 except Exception:
  if time.monotonic()>=deadline:raise
  time.sleep(.25)
text=subprocess.run(['/usr/local/bin/protoc','--proto_path='+str(r/'proto'),'--decode=marengo.v1.SafetyState','marengo/v1/marengo.proto'],input=raw,capture_output=True,check=True).stdout
assert b'mode: OPERATIONAL_MODE_DISABLED' in text
receipt={'label':a.label,'installed_revision':(i/'.deploy-rev').read_text().strip(),'installer_exit':result.returncode,'activation_started_UTC':started,'source_clean':True,'manifest_entries_verified':214,'taught_limits_calibration_environment_preserved':True,'source_owned_backup_SHA256':digest(backup),'physical_motion':'No motor Enable, SetZero or movement test commanded; reference and physical stop remain unqualified.'}
(s/('switch-'+a.label+'-receipt.json')).write_text(json.dumps(receipt,indent=2)+'\n')
print(json.dumps(receipt))
