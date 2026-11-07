"""Netlist checks for rev B: what the sheet is supposed to be, independent of how it was drawn."""
import sys
sys.path.insert(0,sys.argv[1]); from sx import *
N=parse(open(sys.argv[2]).read())
pin={}; nets={}
for net in getall(get(N,'nets'),'net'):
    nm=str(get(net,'name')[1]).lstrip('/'); nodes=[(str(get(x,'ref')[1]),str(get(x,'pin')[1])) for x in getall(net,'node')]
    nets[nm]=nodes
    for k in nodes: pin[k]=nm
comp={str(get(c,'ref')[1]):c for c in getall(get(N,'components'),'comp')}
val=lambda r:str(get(comp[r],'value')[1])
dnp=lambda r:any(str(get(p,'name')[1])=='dnp' for p in getall(comp[r],'property'))
n=lambda r,p:pin.get((r,str(p)))
fail=[]; cnt=[0]
def ok(c,m):
    cnt[0]+=1; print('ok  ' if c else 'FAIL',m)
    if not c: fail.append(m)
def two(a,b,pre=('R','C')):
    return [r for r in comp if r[0] in pre and r[1].isdigit() and {n(r,1),n(r,2)}=={a,b}]
# main path
ok(n('RS1',1)=='BAT' and n('RS1',4)==n('Q1',4)==n('Q2',4),'shunt from BAT to both drains')
ok(all(n(q,p)=='VBUS' for q in ('Q1','Q2') for p in (2,3,5,6,7)),'sources on VBUS: body diodes carry regen to the pack')
ok(n('U1',2)==n('RS1',2) and n('U1',1)==n('RS1',3),'U1 VIN and SENSE Kelvin on RS1')
ok(n('U1',9)=='VBUS' and n('U1',5)=='GND','U1 OUT VBUS')
ok(n('U1',4)=='GND','OVLO low')
ok(two('GATE',n('Q1',1)) and two('GATE',n('Q2',1)),'each gate own R to GATE')
ok(n('U1',3)=='UVLO' and len(two('RUN','UVLO'))==1 and len(two('UVLO','GND'))==1,'UVLO only from RUN: no RUN, no bank')
ok(not any(n(r,p) in('BAT','VBUS') for r,p in pin if r!='x' and n(r,p)=='UVLO'),'UVLO has no path to the bus')
# nothing bulky ahead of the bank
bat=[r for r,p in nets['BAT'] if r[0]=='C']
ok(all(val(c).startswith('1uF') for c in bat),'only 1 uF on the battery side: '+str(bat))
ok(not any(r.startswith('U5') for r,p in nets['BAT']),'brick behind the bank')
ok(n('U5',1)=='VBUS' and n('U5',3)=='GND' and n('U5',2)==n('U1',8),'brick on VBUS, R.C on PGD')
ok(n('U5',4)=='GND' and n('U5',5)=='GND' and n('U5',7)==n('U5',8),'brick -Vout grounded, sense tied')
# latch
ok(n('U2',8)=='BAT' and n('U2',5)=='BAT','LDO on the battery side, EN tied')
ok(n('Q4',2)=='LOOP_RET' and n('Q4',3)=='RUN' and n('Q5',2)=='GND','latch pair')
ok(n('J20',1)=='LOOP_OUT' and n('J20',2)=='GND' and n('J20',3)=='LOOP_RET','loop pins split by GND')
ok(not any(n('J20',p) in('VCTL',) for p in range(1,6)),'no bare 12 V on the loop connector')
ok(two('VCTL','LOOP_OUT') and two('VCTL','RST_SRC'),'loop and reset fed through resistors')
ok(not two('LOOP_RET','RST_IN'),'reset is not fed from the loop return (a held button must not re-arm)')
# clamp
ok(n('U3',3)==n('U4',1) and n('U3',6)=='VCTL','reference on IN-, comparator on VCTL')
ok('Q6' in comp and 'CL_D' in [pin.get(('Q6',str(p))) for p in range(1,10)] and 'CL_G' in [pin.get(('Q6',str(p))) for p in range(1,10)] and 'GND' in [pin.get(('Q6',str(p))) for p in range(1,10)],'clamp FET D, G, S')
ok(len([r for r,p in nets['CL_D'] if r[0]=='J'])==1,'brake terminal on the FET drain')
d=[r for r in comp if r[0]=='D' and val(r).startswith('MBRS')]
ok(len(d)==1,'one freewheel diode')
ok(sum(1 for c in comp if c[0]=='C' and val(c).startswith('100uF'))==4,'four bus caps')
# meters
addr={}
SYM={'GND':0,'+3V3_PI':1,'I2C_SDA':2,'I2C_SCL':3}
for u in [c for c in comp if val(c).startswith('INA228')]:
    a=0x40|(SYM[n(u,1)]<<2)|SYM[n(u,2)]; ok(a not in addr,f'{u} address {a:#x}'); addr[a]=u
    ok(n(u,4)=='I2C_SDA' and n(u,5)=='I2C_SCL' and n(u,6)=='+3V3_PI' and n(u,7)=='GND',u+' bus and supply')
ok(len(addr)==7,'seven meters')
for k in range(2,8):
    r=f'RS{k}'; u=f'U{k+9}'
    ok(n(r,1)=='VBUS' and n(u,10)==n(r,2) and n(u,9)==n(r,3) and n(u,8)==n(r,4),r+' Kelvin to '+u)
legs=[r for r in comp if dnp(r)]
ok(len(legs)==10,'legs DNP: '+str(sorted(legs)))
ok(n('J21',1)=='+3V3_PI' and n('J21',2)=='I2C_SDA' and n('J21',3)=='I2C_SCL' and n('J21',4)=='GND','Pi header')
print('audited',cnt[0]); print('FAILS',len(fail))
