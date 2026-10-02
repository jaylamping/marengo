from pathlib import Path
import datetime, hashlib, json, os, subprocess, sys, time, urllib.request

os.umask(0o077)
stage = Path('/home/joey/marengo-validation/pi-sync-20261002-batch37-comparison')
source = Path('/home/joey/marengo')
installed = Path('/opt/marengo')
main = sys.argv[1]
before = '01a5c403bd8e790cd7403565c777cca8729e48c6'
assert stage.resolve() == stage and source.resolve() == source
assert subprocess.check_output(['git','-C',str(source),'rev-parse','HEAD'],text=True).strip() == main
assert not subprocess.check_output(['git','-C',str(source),'status','--porcelain']).strip()
ci = json.loads((stage/'source-ci.json').read_text(encoding='utf-8-sig'))
assert ci['headSha'] == main and ci['conclusion'] == 'success'
assert len(ci['jobs']) == 5 and all(j['conclusion'] == 'success' for j in ci['jobs'])
assert (installed/'.deploy-rev').read_text().startswith(before+' ')
assert not (stage/'install-candidate.txt').exists()
assert (stage/'limit-preview-receipt.json').is_file()
digest = lambda p: hashlib.sha256(p.read_bytes()).hexdigest()
for name in ('config/motors.yaml','config/control.yaml','assets/urdf/marengo.urdf','var/calibration/zero_registry.yaml'):
    assert digest(installed/name) == digest(stage/'backup'/name), name
assert digest(Path('/etc/marengo/env')) == digest(stage/'backup/runtime.env')
for name, proto in [('safety','SafetyState'),('state','RobotState')]:
    with urllib.request.urlopen('http://127.0.0.1:8080/snapshot/robot/'+name,timeout=5) as response:
        raw = response.read(1024*1024+1)
    assert len(raw) <= 1024*1024
    (stage/f'activation-before-{name}.pb').write_bytes(raw)
    decoded = subprocess.run(['/usr/local/bin/protoc','--proto_path='+str(source/'proto'),
                              '--decode=marengo.v1.'+proto,'marengo/v1/marengo.proto'],
                             input=raw,capture_output=True,check=True).stdout
    (stage/f'activation-before-{name}.txt').write_bytes(decoded)
assert 'mode: OPERATIONAL_MODE_DISABLED' in (stage/'activation-before-safety.txt').read_text()
assert (stage/'activation-before-state.txt').read_text().count('homing_state: JOINT_HOMING_STATE_FAULTED') == 5
for line in (stage/'release-candidate/bundle-sha256.txt').read_text().splitlines():
    expected, name = line.split('  ',1)
    assert digest(source/name) == expected, name
started = datetime.datetime.now(datetime.timezone.utc).isoformat()
(stage/'activation-started.txt').write_text(started+'\n')
for interface in ('can0','can1'):
    (stage/f'activation-before-{interface}.txt').write_bytes(subprocess.check_output(['ip','-details','-statistics','link','show',interface]))
capture_started = time.monotonic()
with (stage/'activation-passive-can.log').open('wb') as output, (stage/'activation-passive-can.stderr').open('wb') as errors:
    observer = subprocess.Popen(['timeout','60','candump','-D','-L',
                                 'can0,0:0,#FFFFFFFF','can1,0:0,#FFFFFFFF'],stdout=output,stderr=errors)
    try:
        ready_deadline = time.monotonic() + 3
        while (stage/'activation-passive-can.log').stat().st_size == 0 and observer.poll() is None and time.monotonic() < ready_deadline: time.sleep(0.02)
        assert observer.poll() is None and (stage/'activation-passive-can.log').stat().st_size > 0 and (stage/'activation-passive-can.stderr').stat().st_size == 0, 'Passive observer must receive frames before installation'
        with (stage/'install-candidate.txt').open('wb') as install_log:
            result = subprocess.run(['sudo','-n',str(source/'scripts/install-pi.sh')],cwd=source,
                                    stdout=install_log,stderr=subprocess.STDOUT,timeout=120)
        (stage/'install-candidate.exit-code').write_text(str(result.returncode)+'\n')
        assert result.returncode == 0, 'Actual installer failed; preserve diagnostics/backup'
        observed_exit = observer.wait(timeout=70)
        assert observed_exit in (0,124), observed_exit
    finally:
        if observer.poll() is None:
            observer.terminate()
            observer.wait(timeout=5)
assert (stage/'activation-passive-can.log').stat().st_size < 8*1024*1024
for name, proto in [('safety','SafetyState'),('state','RobotState')]:
    with urllib.request.urlopen('http://127.0.0.1:8080/snapshot/robot/'+name,timeout=5) as response: raw = response.read(1024*1024+1)
    assert len(raw) <= 1024*1024
    (stage/f'post-{name}.pb').write_bytes(raw)
    decoded = subprocess.run(['/usr/local/bin/protoc','--proto_path='+str(source/'proto'),
                              '--decode=marengo.v1.'+proto,'marengo/v1/marengo.proto'],input=raw,capture_output=True,check=True).stdout
    (stage/f'post-{name}.txt').write_bytes(decoded)
for interface in ('can0','can1'):
    (stage/f'post-{interface}.txt').write_bytes(subprocess.check_output(['ip','-details','-statistics','link','show',interface]))
(stage/'post-services.txt').write_bytes(subprocess.check_output(['systemctl','show','marengo-pi.service','marengo-gateway.service','marengo-can.service','-p','Id','-p','ActiveState','-p','SubState','-p','MainPID','-p','NRestarts']))
(stage/'post-journal.txt').write_bytes(subprocess.check_output(['journalctl','-u','marengo-pi','--since',started,'--no-pager']))
receipt = {'main':main,'activation_started_UTC':started,'installer_exit':0,
           'passive_capture_seconds':time.monotonic()-capture_started,'passive_capture_exit':observed_exit,
           'passive_capture_bytes':(stage/'activation-passive-can.log').stat().st_size,
           'passive_capture_SHA256':digest(stage/'activation-passive-can.log'),
           'scope':'Authorized powered/stable/supported deployment; passive CAN reader only. No Enable, SetZero, target or movement test commanded. Startup/restart is not qualified physical fault recovery.'}
(stage/'activation-receipt.json').write_text(json.dumps(receipt,indent=2)+'\n')
print(json.dumps(receipt))
