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
def diode(k,a):
    # Device:D pin 1 is the cathode
    return [r for r in comp if r[0]=='D' and r[1].isdigit() and n(r,1)==k and n(r,2)==a]
# main path
ok(n('RS1',1)=='BAT' and n('RS1',4)==n('Q8',4)==n('Q9',4),'shunt from BAT to bank A drains')
ok(n('Q1',4)==n('Q2',4)=='MID' and all(n(q,p)=='MID' for q in ('Q8','Q9') for p in (2,3,5,6,7)),'bank A sources feed bank B drains at MID')
ok(all(n(q,p)=='VBUS' for q in ('Q1','Q2') for p in (2,3,5,6,7)),'sources on VBUS: body diodes carry regen to the pack')
ok(n('U1',2)==n('RS1',2) and n('U1',1)==n('RS1',3),'U1 VIN and SENSE Kelvin on RS1')
ok(n('U1',9)=='VBUS' and n('U1',5)=='GND','U1 OUT VBUS')
ok(n('U1',4)=='GND','OVLO low')
ok(two('GATE',n('Q1',1)) and two('GATE',n('Q2',1)),'each gate own R to GATE')
ok(n('U8',15)=='GATE_A' and len(two('G1PD_A','GATE_A',('R',)))==1 and n('U8',13)=='MID' and n('U8',1)==n('U8',3)=='EN_A','U8 drives bank A through a slow pull-down, source on MID, EN and INP1 together')
ok(n('U8',5)=='FLT_A' and two('FLT_A','UVLO',('R',)) and any(r[0]=='D' and n(r,1)=='FLT_A' and n(r,2)=='GND' for r in comp),'U8 FLT holds U1 off until bank A can close, through a resistor, with FLT clamped against a UVLO fault')
ok(n('U8',18)==n('RS1',2) and n('U8',17)==n('RS1',3) and n('U8',8)=='ISCP_A' and n('U8',9)=='TMR_A','bank A short-circuit comparator on RS1')
ok(two('TMR_A','GND',('R',)) and all(dnp(c) for c in two('TMR_A','GND',('C',))),'comparator latches off at its fastest (no TMR cap fitted)')
ok(not ({n('Q8',1),n('Q9',1),'GATE_A'} & {n('Q1',1),n('Q2',1),'GATE'}),'no gate net shared between banks')
ok(len(two('RUN_A','EN_A'))==1 and two('EN_A','GND') and diode('RUN_A','RUN'),'bank A enable from RUN through its own diode, pulled down')
ok(len(two('SW_D','MID',('R',)))==1 and len(two('MID','GND',('R',)))==1,'MID bias')
ok(n('U17',8)=='MID_S' and two('MID','MID_S',('R',)) and n('U17',9)==n('U17',10)=='GND','U17 reads MID through a series resistor')
ok(n('U11',8)=='V_L1','U11 reads VBUS through the L arm output')
ok(n('U1',3)=='UVLO' and len(two('RUN_B','UVLO'))==1 and len(two('UVLO','GND'))==1 and diode('RUN_B','RUN'),'UVLO only from RUN, through its own diode: no RUN, no bank')
ok(not any(r[0]=='R' and 'RUN' in (n(r,1),n(r,2)) and {n(r,1),n(r,2)}&{'UVLO','EN_A'} for r in comp),'no resistor from RUN straight to UVLO or EN_A: a pin fault cannot back-drive RUN')
ok(not any(n(r,p) in('BAT','VBUS') for r,p in pin if r!='x' and n(r,p)=='UVLO'),'UVLO has no path to the bus')
# nothing bulky ahead of the bank
bat=[r for r,p in nets['BAT'] if r[0]=='C']
ok(all(val(c).startswith('1uF') for c in bat),'only 1 uF directly on BAT: '+str(bat))
jb=[r for r in comp if r[0]=='J' and val(r).startswith('12 V converter pigtail')]
ok(len(jb)==1 and not any(r==jb[0] for r,p in nets['BAT']+nets['VBUS']),'converter connector on neither BAT nor VBUS')
jb=jb[0] if jb else 'J0'
ok(all(n(jb,p)!=n(jb,p+6) or n(jb,p)=='GND' for p in range(1,7)) and n(jb,1)==n(jb,2)=='BRK_F' and any(r[0]=='F' and {n(r,1),n(r,2)}=={'BRK_IN','BRK_F'} for r in comp),'converter input behind its soft start and a fuse')
qon=[q for q in comp if q[0]=='Q' and val(q)=='2N7002' and n(q,1)==n('U7',8) and n(q,3)==n(jb,5) and n(q,2)=='GND']
ok(len(qon)==1,'negative-logic ON/OFF pulled to -Vin only when U7 PGD is good')
ok(n('RS9',1)=='BAT' and n('RS9',4)==n('Q7',4) and n('Q7',2)=='BRK_IN','brick path from BAT, ahead of the E-stop bank')
ok(n('U7',2)==n('RS9',2) and n('U7',1)==n('RS9',3) and n('U7',3)==n('U7',2),'U7 Kelvin, UVLO on VIN')
ok(n('U7',4)=='GND' and n('U7',9)=='BRK_IN','U7 OVLO low, OUT on BRK_IN')
ok(not any(r in ('U7','Q7') for r,p in nets['RUN']+nets['VCTL']),'E-stop latch does not touch the brick path')
ok(all(val(c).startswith('2.2uF') for c in [r for r,p in nets['BRK_IN'] if r[0]=='C']),'brick input caps only 2.2 uF')
ok(all(n(jb,p)=='GND' for p in (3,4,6,7,8,9)) and all(n(jb,p)=='+12V_RAW' for p in (10,11,12)) and not {n(jb,p) for p in (1,2)}&{n(jb,p+6) for p in (1,2)}-{'GND'},'converter returns on GND, +Vout on 10-12, nothing on the input row faces +Vout')
# latch
ok(n('U2',8)==n('U2',5)=='BAT_CTL' and any(r[0]=='F' and {n(r,1),n(r,2)}=={'BAT','BAT_CTL'} for r in comp),'LDO on the battery side through a fuse, EN tied')
ok(diode('BAT_CTL','VCTL') and any(r[0]=='D' and n(r,1)=='VCTL' and n(r,2)=='GND' for r in comp),'LDO reverse diode, VCTL clamped')
ok(any(val(c).startswith('10uF') for c in [r for r,p in nets['VCTL'] if r[0]=='C']),'VCTL output cap 10 uF')
ok(n('Q4',2)=='LOOP_RET' and n('Q4',3)=='RUN' and n('Q5',2)=='GND','latch pair')
ok(n('J20',1)=='LOOP_OUT' and n('J20',2)=='GND' and n('J20',3)=='LOOP_RET','loop pins split by GND')
ok(not any(n('J20',p) in('VCTL',) for p in range(1,9)),'no bare 12 V on the loop connector')
ok(n('J20',4)=='GND' and n('J20',5)=='RST_SRC','GND between loop return and reset source')
ok(two('VCTL','LOOP_OUT') and two('VCTL','RST_SRC'),'loop and reset fed through resistors')
ok(not two('LOOP_RET','RST_IN'),'reset is not fed from the loop return (a held button must not re-arm)')
ok(two('RST_SRC','GND',('C',)),'reset source slowed by a cap: reset held at power-up does not arm the latch')
qi=[q for q in comp if q[0]=='Q' and n(q,3)=='RST_SRC' and n(q,2)=='GND']
ok(len(qi)==1 and two('VCTL',n(qi[0],1),('R',)) and any(r[0]=='Q' and n(r,3)==n(qi[0],1) and n(r,1)=='MID_B' for r in comp),'reset interlock: RST_SRC held low until MID is healthy')
# clamp
ok(n('U3',3)==n('U4',1) and n('U3',6)=='VCTL','reference on IN-, comparator on VCTL')
ok('Q6' in comp and 'CL_D' in [pin.get(('Q6',str(p))) for p in range(1,10)] and 'CL_G' in [pin.get(('Q6',str(p))) for p in range(1,10)] and 'GND' in [pin.get(('Q6',str(p))) for p in range(1,10)],'clamp FET D, G, S')
ok(len([r for r,p in nets['CL_D'] if r[0]=='J'])==1,'brake terminal on the FET drain')
d=[r for r in comp if r[0]=='D' and val(r).startswith('MBRS')]
ok(len([r for r in d if n(r,1)=='VBUS' and n(r,2)=='CL_D'])==1,'one freewheel diode')
ok(diode('ABS','BAT') and any(val(c).startswith('47uF') for c in [r for r,p in nets['ABS'] if r[0]=='C']) and two('ABS','GND',('R',)),'battery-side absorber: diode, cap, bleed')
ok(sum(1 for c in comp if c[0]=='C' and val(c).startswith('100uF'))==4,'four bus caps')
# meters
addr={}
SYM={'GND':0,'+3V3_PI':1,'I2C_SDA':2,'I2C_SCL':3}
for u in [c for c in comp if val(c).startswith('INA228')]:
    a=0x40|(SYM[n(u,1)]<<2)|SYM[n(u,2)]; ok(a not in addr,f'{u} address {a:#x}'); addr[a]=u
    ok(n(u,4)=='I2C_SDA' and n(u,5)=='I2C_SCL' and n(u,6)=='+3V3_PI' and n(u,7)=='GND',u+' bus and supply')
ok(len(addr)==9,'nine meters')
for k in range(2,8):
    r=f'RS{k}'; u=f'U{k+9}'
    ok(n(r,1)=='VBUS' and n(u,10)==n(r,2) and n(u,9)==n(r,3) and n(u,8)==n(r,4),r+' Kelvin to '+u)
legs=[r for r in comp if dnp(r) and r!='C29']
ok(n('U9',2)=='SP_EN' and len(two('RUN','SP_EN'))==1 and n('U9',3)==n('U9',4)=='+12V','SPARE_SW eFuse enabled by RUN, fed from +12V')
ok(n('RS10',1)==n('U9',5) and n('U10',10)==n('RS10',2) and n('U10',9)==n('RS10',3) and n('U10',8)==n('RS10',4)=='SPARE_SW','RS10 Kelvin to U10')
ok(n('J22',1)=='SPARE_SW' and n('J22',2)=='GND','SPARE_SW output')
ok(n('J23',1)=='SPARE_ON' and n('J23',2)=='GND' and n('U18',5)=='SPARE_ON' and n('U18',3)==n('U18',4)=='+12V','SPARE_ON output behind its eFuse')
ok(not any(r in ('U18','J23') for r,p in nets['RUN']),'SPARE_ON does not touch the latch')
ok(len(legs)==8,'legs DNP: '+str(sorted(legs)))
ok(len([r for r,p in nets['GND'] if r[0]=='J' and val(r).startswith('GND stud')])==2,'limb returns on two GND studs')
ok(n('J21',1)=='+3V3_PI' and n('J21',2)=='I2C_SDA' and n('J21',3)=='I2C_SCL' and n('J21',4)=='GND','Pi header')
ok(n('J20',8)=='ESTOP_NO' and n('J20',7)=='GND','button NO on the loop connector, GND between it and reset')
ok(two('NO_PU','ESTOP_NO') and diode('NO_PU','+3V3_PI') and two('ESTOP_NO','ESTOP_PI_N') and n('J21',5)=='ESTOP_PI_N','NO pull-up from the Pi rail through a blocking diode, series R to the Pi')
ok(n('D6',3)=='ESTOP_PI_N','clamp on the Pi pin')
ok(not any(r=='J21' for r,p in nets['RUN']+nets['VCTL']+nets['VBUS']),'nothing above 3.3 V on the Pi header')
print('audited',cnt[0]); print('FAILS',len(fail))
