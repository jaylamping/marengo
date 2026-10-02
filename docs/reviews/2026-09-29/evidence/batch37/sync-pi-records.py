from pathlib import Path
import datetime,hashlib,json,sqlite3,subprocess,sys,urllib.request
s=Path('/home/joey/marengo-validation/pi-sync-20261002-batch37-comparison');r=Path('/home/joey/marengo');i=Path('/opt/marengo')
old='0a3fc6bb0dd7ab652d97fd2efb7bb46ceee517c4';new='bab38c6a6e16396b2af7d7d6518eecb4addca96f'
ci=json.loads((s/'main-ci.json').read_text(encoding='utf-8-sig'))
assert ci['headSha']==new and ci['status']=='completed' and ci['conclusion']=='success'
assert len(ci['jobs'])==5 and all(j['conclusion']=='success' for j in ci['jobs'])
assert subprocess.check_output(['git','-C',str(r),'rev-parse','HEAD'],text=True).strip()==old
assert not subprocess.check_output(['git','-C',str(r),'status','--porcelain']).strip()
assert not (s/'source-record-sync-receipt.json').exists()
changes=subprocess.check_output(['git','-C',str(r),'diff','--name-only',old,new],text=True).splitlines()
assert len(changes)==53 and all(n.startswith('docs/') for n in changes)
preparation=json.loads((s/'source-record-sync-preparation.json').read_text())
assert preparation['source_before']==old and preparation['source_after_candidate']==new
backup=s/'backup/source-all-refs-before-PR251-record-sync.bundle'
assert hashlib.sha256(backup.read_bytes()).hexdigest()==preparation['all_ref_backup_verified_SHA256']
hashfile=lambda p:hashlib.sha256(p.read_bytes()).hexdigest()
names=[i/'bin'/n for n in ('marengo-pi','motor-repl','marengo-gateway','marengo-log-cli','imu-probe')]+[i/n for n in ('.deploy-rev','config/motors.yaml','config/control.yaml','assets/urdf/marengo.urdf','var/calibration/zero_registry.yaml')]+[Path('/etc/marengo/env')]
before={str(p):hashfile(p) for p in names}
service_cmd=['systemctl','show','marengo-pi.service','marengo-gateway.service','-p','Id','-p','MainPID','-p','ActiveState','-p','SubState','-p','NRestarts']
services_before=subprocess.check_output(service_cmd,text=True)
subprocess.run(['git','-C',str(r),'merge','--ff-only',new],check=True,capture_output=True)
assert subprocess.check_output(['git','-C',str(r),'rev-parse','HEAD'],text=True).strip()==new
assert not subprocess.check_output(['git','-C',str(r),'status','--porcelain']).strip()
after={str(p):hashfile(p) for p in names};assert after==before
services_after=subprocess.check_output(service_cmd,text=True);assert services_after==services_before
with urllib.request.urlopen('http://127.0.0.1:8080/snapshot/robot/safety',timeout=5) as f:raw=f.read(1048577)
assert len(raw)<=1048576
text=subprocess.run(['/usr/local/bin/protoc','--proto_path='+str(r/'proto'),'--decode=marengo.v1.SafetyState','marengo/v1/marengo.proto'],input=raw,capture_output=True,check=True).stdout
assert b'mode: OPERATIONAL_MODE_DISABLED' in text
(s/'source-record-sync-safety.pb').write_bytes(raw);(s/'source-record-sync-safety.txt').write_bytes(text)
c=sqlite3.connect('file:/opt/marengo/var/marengo.db?mode=ro',uri=True)
try:
 integrity=c.execute('PRAGMA integrity_check').fetchone()[0];assert integrity=='ok'
 schema=json.loads(c.execute("SELECT value_json FROM settings WHERE key='schema_version'").fetchone()[0]);assert schema==3
finally:c.close()
receipt={'recorded_UTC':datetime.datetime.now(datetime.timezone.utc).isoformat(),'source_before':old,'source_after':new,'source_clean':True,'changed_files':len(changes),'all_changes_review_documents':True,'all_non_document_production_inputs_equal_to_qualified_installed_source':True,'installed_release':(i/'.deploy-rev').read_text().strip(),'installed_bin_policy_model_calibration_environment_hashes_unchanged':True,'services_and_PIDs_unchanged':True,'services':services_after,'source_all_refs_backup_verified_SHA256':preparation['all_ref_backup_verified_SHA256'],'Store_integrity':integrity,'Store_schema':schema,'software_mode':'Disabled','physical_motion_acceptance':False,'scope':'Docs-only Pi source fast-forward. No installer, service restart, CAN write, motor enable, SetZero, target or movement test. Qualified runtime remains0a3fc6b with identical production inputs.'}
(s/'source-record-sync-receipt.json').write_text(json.dumps(receipt,indent=2)+'\n');print(json.dumps(receipt))