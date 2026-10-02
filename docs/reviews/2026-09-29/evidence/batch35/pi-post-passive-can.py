from pathlib import Path
import datetime,hashlib,json,re,subprocess,time,urllib.request
stage=Path('/home/joey/marengo-validation/pi-sync-20261002-batch35')
assert stage.resolve()==stage
out=stage/'post-passive-can.log'; err=stage/'post-passive-can.stderr'
assert not out.exists() and not err.exists()
started=datetime.datetime.now(datetime.timezone.utc).isoformat()
before={}
for interface in ('can0','can1'):
    base=Path('/sys/class/net')/interface/'statistics'
    before[interface]={p.name:int(p.read_text()) for p in base.iterdir() if p.is_file()}
with out.open('wb') as capture, err.open('wb') as errors:
    result=subprocess.run(['timeout','30','candump','-D','-L','can0,0:0,#FFFFFFFF','can1,0:0,#FFFFFFFF'],stdout=capture,stderr=errors,timeout=35)
assert result.returncode in (0,124), result.returncode
assert out.stat().st_size<8*1024*1024
lines=out.read_text().splitlines()
assert len(lines)>=50, 'Passive observer must actually receive data'
parsed=[]
for line in lines:
    match=re.fullmatch(r'\((\d+\.\d+)\)\s+(can[01])\s+([0-9A-Fa-f]+)#([0-9A-Fa-f]*)',line)
    assert match, line
    parsed.append((float(match[1]),match[2],int(match[3],16),match[4]))
error_lines=[line for line,row in zip(lines,parsed) if row[2]&0x20000000]
(stage/'post-passive-can-error-frames.txt').write_text('\n'.join(error_lines)+('\n' if error_lines else ''))
(stage/'post-passive-can-sample.txt').write_text('\n'.join(lines[:50])+'\n')
after={}
for interface in ('can0','can1'):
    base=Path('/sys/class/net')/interface/'statistics'
    after[interface]={p.name:int(p.read_text()) for p in base.iterdir() if p.is_file()}
counts={}
for _,interface,can_id,_ in parsed:
    key=interface+'/'+format(can_id,'08X'); counts[key]=counts.get(key,0)+1
snapshots=[]
for i in range(3):
    item={'captured_UTC':datetime.datetime.now(datetime.timezone.utc).isoformat()}
    for name,proto in [('safety','SafetyState'),('state','RobotState')]:
        with urllib.request.urlopen('http://127.0.0.1:8080/snapshot/robot/'+name,timeout=5) as response: raw=response.read(1024*1024+1)
        assert len(raw)<=1024*1024
        decoded=subprocess.run(['/usr/local/bin/protoc','--proto_path=/home/joey/marengo/proto','--decode=marengo.v1.'+proto,'marengo/v1/marengo.proto'],input=raw,capture_output=True,check=True).stdout
        (stage/f'post-persistent-{i}-{name}.pb').write_bytes(raw)
        (stage/f'post-persistent-{i}-{name}.txt').write_bytes(decoded)
        if name=='safety':
            item['active_fault_count']=decoded.count(b'active_faults {')
            item['software_latch']=b'software_estop_latched: true' in decoded
            item['disabled']=b'mode: OPERATIONAL_MODE_DISABLED' in decoded
            assert item['active_fault_count']==1 and item['software_latch'] and item['disabled']
        else:
            item['faulted_joint_count']=decoded.count(b'homing_state: JOINT_HOMING_STATE_FAULTED')
            assert item['faulted_joint_count']==5
    snapshots.append(item)
    if i<2: time.sleep(0.16)
receipt={'started_UTC':started,'duration_seconds':30,'capture_exit':result.returncode,'capture_SHA256':hashlib.sha256(out.read_bytes()).hexdigest(),'capture_bytes':out.stat().st_size,'data_and_error_frames':len(parsed),'error_frame_count':len(error_lines),'frame_counts':counts,'capture_stderr':err.read_text(),'before_statistics':before,'after_statistics':after,'persistent_live_publications':snapshots,'scope':'Read-only passive CAN and existing snapshot reads after the natural fault; no movement or fault injection. This post-fault capture cannot establish the initiating error envelope.'}
(stage/'post-passive-can-receipt.json').write_text(json.dumps(receipt,indent=2)+'\n')
print(json.dumps(receipt))
