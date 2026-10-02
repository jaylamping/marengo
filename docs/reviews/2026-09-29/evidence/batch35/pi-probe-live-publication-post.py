from pathlib import Path
import datetime,json,subprocess,urllib.request
output=Path('/home/joey/marengo-validation/batch35-live-fault-publication-post')
output.mkdir(mode=0o700,exist_ok=True)
values={}
for name,proto in [('safety','SafetyState'),('state','RobotState')]:
 with urllib.request.urlopen('http://127.0.0.1:8080/snapshot/robot/'+name,timeout=5) as response: raw=response.read(1024*1024+1)
 assert len(raw)<=1024*1024
 decoded=subprocess.run(['/usr/local/bin/protoc','--proto_path=/home/joey/marengo/proto','--decode=marengo.v1.'+proto,'marengo/v1/marengo.proto'],input=raw,capture_output=True,check=True).stdout
 (output/f'{name}.pb').write_bytes(raw);(output/f'{name}.txt').write_bytes(decoded)
 values[name]=decoded.decode()
print(json.dumps({'captured_UTC':datetime.datetime.now(datetime.timezone.utc).isoformat(),'joint_faults':values['state'].count('fault: 1'),'faulted_references':values['state'].count('homing_state: JOINT_HOMING_STATE_FAULTED'),'SafetyState_active_faults':values['safety'].count('active_faults {'),'SafetyState_software_latch_reported':'software_estop_latched: true' in values['safety']}))
assert values['state'].count('homing_state: JOINT_HOMING_STATE_FAULTED')==5,'Require the actually latched live fault as repro input'
assert 'active_faults {' in values['safety'],'Latched authority disappeared from actual SafetyState after healthy ticks'
assert 'software_estop_latched: true' in values['safety'],'Latched authority is falsely published clear'
