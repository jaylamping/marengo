from pathlib import Path
import argparse,datetime,hashlib,json,os,re,subprocess,time,urllib.error,urllib.request
os.umask(0o077)
parser=argparse.ArgumentParser()
parser.add_argument('--label',required=True,choices=('B1','A2','B2'))
parser.add_argument('--revision',required=True,choices=('01a5c403bd8e790cd7403565c777cca8729e48c6','0a3fc6bb0dd7ab652d97fd2efb7bb46ceee517c4'))
parser.add_argument('--seconds',required=True,type=int,choices=(60,300))
args=parser.parse_args()
stage=Path('/home/joey/marengo-validation/batch37-can-comparison-20261002-'+args.label)
assert stage.parent.resolve()==stage.parent
stage.mkdir(mode=0o700)
main=args.revision
assert Path('/opt/marengo/.deploy-rev').read_text().startswith(main+' ')
proto='/home/joey/marengo/proto'
def snapshot(label):
    item={'captured_UTC':datetime.datetime.now(datetime.timezone.utc).isoformat()}
    for name,kind in [('safety','SafetyState'),('state','RobotState')]:
        try:
            with urllib.request.urlopen('http://127.0.0.1:8080/snapshot/robot/'+name,timeout=3) as response: raw=response.read(1024*1024+1)
            assert len(raw)<=1024*1024
            decoded=subprocess.run(['/usr/local/bin/protoc','--proto_path='+proto,'--decode=marengo.v1.'+kind,'marengo/v1/marengo.proto'],input=raw,capture_output=True,check=True).stdout
            (stage/f'{label}-{name}.pb').write_bytes(raw)
            (stage/f'{label}-{name}.txt').write_bytes(decoded)
            if name=='safety':
                item['active_fault_count']=decoded.count(b'active_faults {')
                item['software_latch']=b'software_estop_latched: true' in decoded
                item['disabled']=b'mode: OPERATIONAL_MODE_DISABLED' in decoded
                match=re.search(rb'timestamp_ms: (\d+)',decoded);item['safety_timestamp_ms']=int(match[1]) if match else None
            else: item['faulted_joint_count']=decoded.count(b'homing_state: JOINT_HOMING_STATE_FAULTED')
        except urllib.error.HTTPError as err: item[name+'_HTTP_error']=err.code
    return item
before=snapshot('before')
assert before['disabled'], 'Observation requires software Disabled before restart'
policy_names=('config/motors.yaml','config/control.yaml','assets/urdf/marengo.urdf','var/calibration/zero_registry.yaml')
policy_hashes={name:hashlib.sha256((Path('/opt/marengo')/name).read_bytes()).hexdigest() for name in policy_names}
env_hash=hashlib.sha256(Path('/etc/marengo/env').read_bytes()).hexdigest()
def statistics():
    values={}
    for name in ('can0','can1'):
        base=Path('/sys/class/net')/name/'statistics'
        values[name]={p.name:int(p.read_text()) for p in base.iterdir() if p.is_file()}
    return values
before_stats=statistics()
for label,cmd in [('before-services',['systemctl','show','marengo-pi.service','marengo-gateway.service','-p','Id','-p','MainPID','-p','ActiveState','-p','NRestarts']),('before-can0',['ip','-details','-statistics','link','show','can0']),('before-can1',['ip','-details','-statistics','link','show','can1'])]: (stage/(label+'.txt')).write_bytes(subprocess.check_output(cmd))
irq_metadata={}
for number in ('108','119','171','172'):
    path=Path('/proc/irq')/number
    irq_metadata[number]={name:(path/name).read_text().strip() for name in ('smp_affinity_list','effective_affinity_list') if (path/name).is_file()}
spi={}
for name in ('spi0.0','spi0.1'):
    device=Path('/sys/bus/spi/devices')/name
    speed=device/'of_node/spi-max-frequency'
    spi[name]={'driver':str((device/'driver').resolve()),'configured_spi_max_frequency_hz':int.from_bytes(speed.read_bytes(),'big') if speed.is_file() else None}
started=datetime.datetime.now(datetime.timezone.utc).isoformat()
monotonic_start=time.monotonic()
raw_path=stage/'passive-can.log';err_path=stage/'passive-can.stderr'
samples=[]
with raw_path.open('wb') as output,err_path.open('wb') as errors:
    observer=subprocess.Popen(['timeout',str(args.seconds),'candump','-D','-L','can0,0:0,#FFFFFFFF','can1,0:0,#FFFFFFFF'],stdout=output,stderr=errors)
    try:
        ready_deadline=time.monotonic()+3
        while raw_path.stat().st_size==0 and observer.poll() is None and time.monotonic()<ready_deadline: time.sleep(0.02)
        assert observer.poll() is None and raw_path.stat().st_size>0 and err_path.stat().st_size==0,'Passive observer must receive valid frames before restart'
        ready_bytes=raw_path.stat().st_size
        restart_started=datetime.datetime.now(datetime.timezone.utc).isoformat()
        with (stage/'restart.txt').open('wb') as log:
            result=subprocess.run(['sudo','-n','/usr/local/libexec/marengo/pi-restart-marengo-pi.sh','restart'],stdout=log,stderr=subprocess.STDOUT,timeout=30)
        (stage/'restart.exit-code').write_text(str(result.returncode)+'\n')
        assert result.returncode==0,'Retain failed restart evidence; do not retry automatically'
        i=0
        while observer.poll() is None:
            item=snapshot(f'sample-{i:02d}')
            item['can0_rx_over_errors']=int(Path('/sys/class/net/can0/statistics/rx_over_errors').read_text())
            item['loadavg']=Path('/proc/loadavg').read_text().strip()
            item['cpu0_stat']=next(line for line in Path('/proc/stat').read_text().splitlines() if line.startswith('cpu0 '))
            item['CAN_IRQ_counts']=[line for line in Path('/proc/interrupts').read_text().splitlines() if 'spi0.' in line]
            samples.append(item);i+=1
            time.sleep(1)
        observed_exit=observer.wait(timeout=5)
        assert observed_exit in (0,124),observed_exit
    finally:
        if observer.poll() is None:
            observer.terminate();observer.wait(timeout=5)
assert raw_path.stat().st_size<24*1024*1024
lines=raw_path.read_text().splitlines();events=[];counts={}
for line in lines:
    match=re.fullmatch(r'\((\d+\.\d+)\)\s+(can[01])\s+([0-9A-Fa-f]+)#([0-9A-Fa-f]*)',line)
    assert match,line
    timestamp=float(match[1]);interface=match[2];can_id=int(match[3],16)
    key=interface+'/'+format(can_id,'08X');counts[key]=counts.get(key,0)+1
    if can_id&0x20000000:
        events.append({'timestamp_UTC':datetime.datetime.fromtimestamp(timestamp,datetime.timezone.utc).isoformat(),'interface':interface,'CAN_ID_hex':format(can_id,'08X'),'CAN_error_class_mask_hex':format(can_id&0x1FFFFFFF,'08X'),'payload_length':len(match[4])//2,'payload_hex':match[4],'raw_line':line})
(stage/'passive-error-frames.txt').write_text('\n'.join(e['raw_line'] for e in events)+('\n' if events else ''))
(stage/'passive-sample.txt').write_text('\n'.join(lines[:50])+'\n')
for label,cmd in [('after-services',['systemctl','show','marengo-pi.service','marengo-gateway.service','-p','Id','-p','MainPID','-p','ActiveState','-p','NRestarts']),('after-can0',['ip','-details','-statistics','link','show','can0']),('after-can1',['ip','-details','-statistics','link','show','can1']),('runtime-journal',['journalctl','-u','marengo-pi','--since',started,'--no-pager'])]: (stage/(label+'.txt')).write_bytes(subprocess.check_output(cmd))
for name,expected in policy_hashes.items(): assert hashlib.sha256((Path('/opt/marengo')/name).read_bytes()).hexdigest()==expected,name
assert hashlib.sha256(Path('/etc/marengo/env').read_bytes()).hexdigest()==env_hash
receipt={'comparison_label':args.label,'requested_seconds':args.seconds,'installed_binary_sha256':{p.name:hashlib.sha256(p.read_bytes()).hexdigest() for p in Path('/opt/marengo/bin').iterdir() if p.is_file()},'policy_sha256':policy_hashes,'started_UTC':started,'observer_confirmed_ready_bytes_before_restart':ready_bytes,'restart_started_UTC':restart_started,'restart_exit':0,'capture_elapsed_seconds':time.monotonic()-monotonic_start,'passive_capture_exit':observed_exit,'capture_bytes':raw_path.stat().st_size,'capture_SHA256':hashlib.sha256(raw_path.read_bytes()).hexdigest(),'capture_stderr':err_path.read_text(),'frame_count':len(lines),'frame_counts':counts,'error_frames':events,'before_statistics':before_stats,'after_statistics':statistics(),'before_snapshot':before,'snapshot_samples':samples,'policy_calibration_and_runtime_environment_unchanged':True,'IRQ_affinity_observed':irq_metadata,'SPI_devices':spi,'kernel':subprocess.check_output(['uname','-r'],text=True).strip(),'installed_revision':Path('/opt/marengo/.deploy-rev').read_text().strip(),'scope':'One authorized software-only diagnostic restart, passive listener ready before stop/start. Startup remains Disabled/Unhomed; no Enable, SetZero, target, motion test or fault injection commanded. Restart is not qualified physical fault recovery; physical stop remains unconfirmed. Captured error evidence establishes observed frame bytes/time, not the cause of servicing delay.'}
(stage/'diagnostic-receipt.json').write_text(json.dumps(receipt,indent=2)+'\n')
print(json.dumps({key:receipt[key] for key in ('restart_exit','frame_count','error_frames','capture_SHA256')}))
