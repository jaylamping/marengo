from pathlib import Path
import datetime,hashlib,json,os,re,subprocess,time,urllib.request
stage=Path('/home/joey/marengo-validation/batch38-passive-owner-20261002')
assert stage.resolve()==stage and not stage.exists()
os.umask(0o077); stage.mkdir(mode=0o700)
source=Path('/home/joey/marengo')
installed=Path('/opt/marengo')
revision=(installed/'.deploy-rev').read_text().strip()
assert revision.startswith('2d0fd4088b0ca0301b2e27b07560b54b40ec61ca ')
pid=subprocess.check_output(['systemctl','show','marengo-pi.service','-p','MainPID','--value'],text=True).strip()
assert pid!='0'
(stage/'before-can0.txt').write_bytes(subprocess.check_output(['ip','-details','-statistics','link','show','can0']))
started=datetime.datetime.now(datetime.timezone.utc).isoformat()
samples=[]; changes=[]; prior=None
capture=stage/'passive-can.log'; stderr=stage/'passive-can.stderr'
with capture.open('wb') as output,stderr.open('wb') as errors:
    observer=subprocess.Popen(['timeout','300','candump','-D','-L','can0,0:0,#FFFFFFFF','can1,0:0,#FFFFFFFF'],stdout=output,stderr=errors)
    try:
        ready=time.monotonic()+3
        while capture.stat().st_size==0 and observer.poll() is None and time.monotonic()<ready: time.sleep(.02)
        assert observer.poll() is None and capture.stat().st_size>0 and stderr.stat().st_size==0
        print('Passive300s owner capture started; no restart, build or motor command',flush=True)
        deadline=time.monotonic()+310
        while observer.poll() is None:
            assert time.monotonic()<deadline
            assert capture.stat().st_size<=24*1024*1024
            with urllib.request.urlopen('http://127.0.0.1:8080/snapshot/robot/safety',timeout=5) as response: raw=response.read(1024*1024+1)
            assert len(raw)<=1024*1024
            decoded=subprocess.run(['/usr/local/bin/protoc','--proto_path='+str(source/'proto'),'--decode=marengo.v1.SafetyState','marengo/v1/marengo.proto'],input=raw,capture_output=True,check=True).stdout
            text=decoded.decode()
            assert 'mode: OPERATIONAL_MODE_DISABLED' in text
            timestamp=int(re.search(r'^timestamp_ms: (\d+)$',text,re.M).group(1))
            assert 0<=int(time.time()*1000)-timestamp<5000
            messages=re.findall(r'^\s*message: (".*")$',text,re.M)
            sample={'observed_ms':int(time.time()*1000),'producer_ms':timestamp,'disabled':True,'software_latch':'software_estop_latched: true' in text,'messages':messages}
            samples.append(sample)
            if messages!=prior:
                index=len(changes)
                assert index<32
                (stage/f'change-{index:02}-safety.pb').write_bytes(raw)
                (stage/f'change-{index:02}-safety.txt').write_bytes(decoded)
                changes.append({'sample_index':len(samples)-1,'files':f'change-{index:02}-safety','messages':messages})
                prior=messages
            time.sleep(1)
        status=observer.wait(timeout=5)
        assert status in (0,124)
    finally:
        if observer.poll() is None: observer.terminate();observer.wait(timeout=5)
assert not stderr.read_bytes()
assert (installed/'.deploy-rev').read_text().strip()==revision
assert subprocess.check_output(['systemctl','show','marengo-pi.service','-p','MainPID','--value'],text=True).strip()==pid
(stage/'after-can0.txt').write_bytes(subprocess.check_output(['ip','-details','-statistics','link','show','can0']))
(stage/'journal.txt').write_bytes(subprocess.check_output(['journalctl','-u','marengo-pi','--since',started,'--no-pager']))
r={'started_UTC':started,'ended_UTC':datetime.datetime.now(datetime.timezone.utc).isoformat(),'installed_revision':revision,'owner_PID_unchanged':pid,'capture_exit':status,'capture_SHA256':hashlib.sha256(capture.read_bytes()).hexdigest(),'samples':samples,'changes':changes,'scope':'Passive existing owner/CAN observation; no restart, build, load injection, motor command or physical acceptance'}
(stage/'passive-owner-receipt.json').write_text(json.dumps(r,indent=2)+'\n')
print(json.dumps({'complete_samples':len(samples),'changes':len(changes),'capture_SHA256':r['capture_SHA256'],'PID':pid}),flush=True)