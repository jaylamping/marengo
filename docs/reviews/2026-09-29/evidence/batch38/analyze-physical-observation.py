from pathlib import Path
import collections,hashlib,json,re
work=Path('J:/code/marengo-migration-backup-20260929/batch38-pi-receive-diagnostics')
obs=work/'pi-observation'
raw=(obs/'activation-passive-can.log').read_bytes()
activation=json.loads((obs/'activation-receipt.json').read_text())
assert hashlib.sha256(raw).hexdigest()==activation['passive_capture_SHA256']
assert activation['installer_exit']==0 and activation['passive_capture_exit'] in (0,124)
assert not (obs/'activation-passive-can.stderr').read_bytes()
frames=[]
for line in raw.decode().splitlines():
    m=re.fullmatch(r'\((\d+\.\d+)\)\s+(can[01])\s+([0-9A-Fa-f]+)#([0-9A-Fa-f]*)',line)
    assert m,line
    assert len(m[4])<=16 and len(m[4])%2==0,line
    frames.append((float(m[1]),m[2],int(m[3],16),m[4],line))
assert frames and frames[-1][0]-frames[0][0]>=55
errors=[f[4] for f in frames if f[2]&0x20000000]
kind_counts=collections.Counter()
neutral=[]
for _,interface,can_id,data,line in frames:
    if can_id&0x20000000: continue
    kind=(can_id>>24)&31
    kind_counts[kind]+=1
    assert kind not in (3,6),line
    if kind==1:
        assert data=='7FFF7FFF00000000',line
        neutral.append(line)
def counter(filename):
    text=(obs/filename).read_text()
    m=re.search(r'RX:\s+bytes\s+packets\s+errors\s+dropped\s+missed\s+mcast\s+\n\s*\d+\s+\d+\s+(\d+)',text)
    assert m,filename
    return int(m[1])
pre=json.loads((obs/'update-preparation.json').read_text())
fresh=json.loads((obs/'fresh-live-verification.json').read_text())
installed=json.loads((obs/'post-install-file-verification.json').read_text())
assert installed['manifest_entries_verified']==214 and installed['all_five_joint_limits_and_motor_identity_preserved']
assert installed['runtime_environment_unchanged'] and installed['Store_integrity_check']=='ok' and installed['Store_schema_version']==3
r={'source':activation['candidate_source'],'installer_exit':0,'captured_seconds':frames[-1][0]-frames[0][0],'frames':len(frames),'capture_SHA256':hashlib.sha256(raw).hexdigest(),'error_frames':errors,'transport_frame_kinds':dict(sorted(kind_counts.items())),'neutral_MIT_frames':len(neutral),'Enable_and_SetZero_frames':0,'CAN0_rx_over_errors':{'at_backup':counter('update-before-can0.txt'),'before_install':counter('activation-before-can0.txt'),'after_capture':counter('post-can0.txt'),'fresh':fresh['CAN0_rx_over_errors']},'before_update_latch':pre['live_before'],'fresh_state':{'mode':fresh['mode'],'Faulted':fresh['faulted_joints'],'Unhomed':fresh['unhomed_joints'],'software_latch':fresh['software_latch']},'release_verified':installed['manifest_entries_verified'],'taught_limits_identity_calibration_environment_preserved':True,'Store_integrity_schema':'ok/3','HTTPS':'200, local certificate explicitly trusted, exact installed index','owner_raw_frame_hardware_comparison':'No new error captured in this bounded window; physical owner/error correlation remains unqualified' if not errors else 'New error captured; owner correlation requires separate ordered analysis','physical_acceptance':False,'limits':'Software restart is not physical fault recovery. Earlier CAN recurrence remains, physical current reference/priority stop and movement tests remain unqualified. No reply pending.'}
(work/'physical-observation.json').write_text(json.dumps(r,indent=2)+'\n',encoding='utf-8')
print(json.dumps(r,indent=2))