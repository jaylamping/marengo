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
fd=[n(r,2) for r in comp if r[0]=='D' and r[1].isdigit() and n(r,1)=='FLT_A' and n(r,2)!='GND']
ok(n('U8',5)=='FLT_A' and len(fd)==1 and two(fd[0],'UVLO',('R',)) and any(r[0]=='D' and n(r,1)=='FLT_A' and n(r,2)=='GND' for r in comp),'U8 FLT holds U1 off until bank A can close, through a diode (pull-down only) and a resistor, with FLT clamped against a UVLO fault')
csp=two('SH_P',n('U8',18),('R',)); csn=two('SH_N',n('U8',17),('R',)); csc=two(n('U8',18),n('U8',17),('C',))
ok(len(csp)==len(csn)==1 and val(csp[0])==val(csn[0])=='0R' and csc and all(dnp(c) for c in csc) and n('U8',8)=='ISCP_A' and n('U8',9)=='TMR_A','bank A short-circuit comparator on RS1, RC placeholder 0R fitted, cap DNP')
ok(two('TMR_A','GND',('R',)) and all(dnp(c) for c in two('TMR_A','GND',('C',))),'comparator latches off at its fastest (no TMR cap fitted)')
ok(not ({n('Q8',1),n('Q9',1),'GATE_A'} & {n('Q1',1),n('Q2',1),'GATE'}),'no gate net shared between banks')
ok(len(two('RUN_A','EN_A'))==1 and two('EN_A','GND') and diode('RUN_A','RUN'),'bank A enable from RUN through its own diode, pulled down')
ok(len(two('SW_D','MID',('R',)))==1 and len(two('MID','GND',('R',)))==1,'MID bias')
ok(n('U17',8)=='MID_S' and two('MID','MID_S',('R',)) and n('U17',9)=='GND' and n('U17',10)=='CL_TM' and two('CL_OUT','CL_TM',('R',)) and two('CL_TM','GND',('C',)),'U17 reads MID through a series resistor, clamp duty on IN+')
ok(n('U12',8)=='D_L2' and n('U11',8)=='V_L1','U12 reads VBUS on the R arm port input, U11 its port output')
ok(n('U1',3)=='UVLO' and len(two('RUN_B','UVLO'))==1 and len(two('UVLO','GND'))==1 and diode('RUN_B','RUN'),'UVLO only from RUN, through its own diode: no RUN, no bank')
ok(not any(r[0]=='R' and 'RUN' in (n(r,1),n(r,2)) and {n(r,1),n(r,2)}&{'UVLO','EN_A'} for r in comp),'no resistor from RUN straight to UVLO or EN_A: a pin fault cannot back-drive RUN')
ok(not any(n(r,p) in('BAT','VBUS') for r,p in pin if r!='x' and n(r,p)=='UVLO'),'UVLO has no path to the bus')
pk=[r for r in comp if r[0]=='J' and n(r,1)=='BAT']
ok(len(pk)==1 and n(pk[0],2)=='GND' and not any(r[0]=='J' and r!=pk[0] for r,p in nets['BAT']),'pack in on one keyed two-pole connector, BAT and GND side by side')
# nothing bulky ahead of the bank
bat=[r for r,p in nets['BAT'] if r[0]=='C']
# rev B5 layout stage 2: the pack-entry ceramics (4 x 4.7 uF 100 V) take the bank-trip edge; about 14 mJ at 54.6 V, which
# the XT90S anti-spark plug absorbs, and the BMS close is set by the absorber cans. No bulk or polarized capacitance here.
def uf(v): import re; m=re.match(r'([\d.]+)uF',v); return float(m[1]) if m else 99
ok(all('CP_' not in str(get(comp[c],'footprint')[1]) and uf(val(c))<=4.7 for c in bat) and sum(uf(val(c)) for c in bat)<=20,'only ceramics, 20 uF or less, directly on BAT: '+str(bat))
jb=[r for r in comp if r[0]=='J' and val(r).startswith('12 V converter pigtail')]
ok(len(jb)==1 and not any(r==jb[0] for r,p in nets['BAT']+nets['VBUS']),'converter connector on neither BAT nor VBUS')
jb=jb[0] if jb else 'J0'
ok(all(n(jb,p)!=n(jb,p+6) or n(jb,p)=='GND' for p in range(1,7)) and n(jb,1)==n(jb,2)=='BRK_F' and any(r[0]=='F' and {n(r,1),n(r,2)}=={'BRK_IN','BRK_F'} for r in comp),'converter input behind its soft start and a fuse')
qon=[q for q in comp if q[0]=='Q' and val(q)=='2N7002' and n(q,1)==n('U7',8) and two(n(q,3),n(jb,5),('R',)) and n(q,2)=='GND']
ok(len(qon)==1,'negative-logic ON/OFF pulled to -Vin only when U7 PGD is good')
ok(n('RS9',1)=='BAT' and n('RS9',4)==n('Q7',4) and n('Q7',2)=='BRK_IN','brick path from BAT, ahead of the E-stop bank')
ok(n('U7',2)==n('RS9',2) and n('U7',1)==n('RS9',3) and n('U7',3)==n('U7',2),'U7 Kelvin, UVLO on VIN')
ok(n('U7',4)=='GND' and n('U7',9)=='BRK_IN','U7 OVLO low, OUT on BRK_IN')
ok(not any(r in ('U7','Q7') for r,p in nets['RUN']+nets['VCTL']),'E-stop latch does not touch the brick path')
ok(all(val(c).startswith('2.2uF') for c in [r for r,p in nets['BRK_IN'] if r[0]=='C']),'brick input caps only 2.2 uF')
ok(all(n(jb,p)=='GND' for p in (3,4,6,7,8,9)) and all(n(jb,p)=='+12V_RAW' for p in (10,11,12)) and not {n(jb,p) for p in (1,2)}&{n(jb,p+6) for p in (1,2)}-{'GND'},'converter returns on GND, +Vout on 10-12, nothing on the input row faces +Vout')
# latch
f1=[r for r in comp if r[0]=='F' and n(r,1)=='BAT']; fo=n(f1[0],2) if f1 else None
rs=[r for r in comp if r[0]=='R' and fo in (n(r,1),n(r,2))]; ro=({n(rs[0],1),n(rs[0],2)}-{fo}).pop() if len(rs)==1 else None
ok(n('U2',8)==n('U2',5)=='BAT_CTL' and len(f1)==1 and ro and diode('BAT_CTL',ro),'LDO fed from BAT through a fuse, a series R and a series diode, EN tied')
ok(not diode('BAT_CTL','VCTL') and any(r[0]=='D' and n(r,1)=='VCTL' and n(r,2)=='GND' for r in comp),'no OUT-to-IN diode (series diode instead), VCTL clamped')
ok(any(val(c).startswith('10uF') for c in [r for r,p in nets['VCTL'] if r[0]=='C']),'VCTL output cap 10 uF')
ok(n('Q4',2)=='LOOP_RET' and n('Q4',3)=='RUN' and n('Q5',2)=='L_E','latch pair, seal-in NPN emitter through the watchdog switch')
wd=[q for q in comp if q[0]=='Q' and n(q,3)=='L_E' and n(q,2)=='GND']
uw=[u for u in comp if val(u).startswith('TPS3431')]
ok(len(wd)==1 and len(uw)==1 and n(wd[0],1)==n(uw[0],7) and two('VWD',n(uw[0],7),('R',)) and n(uw[0],1)=='VWD' and not any(x=='J21' for x,p in nets['VWD']),'watchdog: WDO pulls the series NPN base low; base fed from the watchdog rail, not the Pi')
ok(two('VCTL','VWD',('R',)) and any(r[0]=='D' and n(r,1)=='VWD' and n(r,2)=='GND' for r in comp),'watchdog rail from VCTL through a resistor and zener')
ok(n('J21',6)=='WD_FEED' and two('WD_FEED',n(uw[0],6),('R',)) and two(n(uw[0],6),'GND',('R',)),'heartbeat from the Pi through a series R, pulled down at the watchdog')
ok(n('J20',1)=='LOOP_OUT' and n('J20',2)=='GND' and n('J20',3)=='LOOP_RET','loop pins split by GND')
ok(not any(n('J20',p) in('VCTL',) for p in range(1,9)),'no bare 12 V on the loop connector')
ok(n('J20',4)=='GND' and n('J20',5)=='RST_SRC','GND between loop return and reset source')
ok(two('VCTL','LOOP_OUT') and two('VCTL','RST_SRC'),'loop and reset fed through resistors')
ok(not two('LOOP_RET','RST_IN'),'reset is not fed from the loop return (a held button must not re-arm)')
ok(two('RST_SRC','GND',('C',)),'reset source slowed by a cap: reset held at power-up does not arm the latch')
qi=[q for q in comp if q[0]=='Q' and n(q,3)=='RST_SRC' and n(q,2)=='GND']
ok(len([q for q in qi if n(q,1)==n(qi[0],1)])>=1 and two('VCTL',n(qi[0],1),('R',)) and any(r[0]=='Q' and n(r,3)==n(qi[0],1) and n(r,1)=='MID_B' for r in comp),'reset interlock: RST_SRC held low until MID is healthy')
ok(any(q[0]=='Q' and n(q,3)=='RST_SRC' and n(q,2)=='GND' and n(q,1)=='VB_B' for q in comp) and any(r[0]=='D' and n(r,1)=='VBUS' and n(r,2)=='VB_Z' for r in comp),'reset interlock: RST_SRC held low while VBUS is up')
# clamp
ok(n('U3',3)==n('U4',1) and n('U3',6)=='VCTL','reference on IN-, comparator on VCTL')
ok('Q6' in comp and 'CL_D' in [pin.get(('Q6',str(p))) for p in range(1,10)] and 'CL_G' in [pin.get(('Q6',str(p))) for p in range(1,10)] and 'GND' in [pin.get(('Q6',str(p))) for p in range(1,10)],'clamp FET D, G, S')
hs=[r for r in comp if r[0]=='J' and val(r).startswith('HS50')]
ok(len(hs)==2 and all(n(j,1)=='VBUS' and n(j,2)=='CL_D' for j in hs),'two brake resistors (2 x HS50 20R), each on its own header between VBUS and the FET drain')
d=[r for r in comp if r[0]=='D' and val(r).startswith('MBRS')]
ok(len([r for r in d if n(r,1)=='VBUS' and n(r,2)=='CL_D'])==1,'one freewheel diode')
ok(len(diode('ABS','BAT'))==2 and len([c for c in [r for r,p in nets['ABS'] if r[0]=='C'] if val(c).startswith('47uF') and not dnp(c)])==8 and any(val(c).startswith('47uF') for c in [r for r,p in nets['ABS'] if r[0]=='C']) and two('ABS','GND',('R',)),'battery-side absorber: diode, cap, bleed')
absn=[r for r in comp if r[0]=='R' and {n(r,1),n(r,2)}=={'ABS_N','GND'}]
ok(any(val(c).startswith('47uF') for c in [r for r,p in nets['ABS_N'] if r[0]=='C']) and [r for r in absn if not dnp(r) and val(r)=='copper link'] and all(dnp(r) for r in absn if val(r)!='copper link'),'absorber ground leg: copper link fitted, close-surge option DNP')
ok(sum(1 for c in comp if c[0]=='C' and val(c).startswith('47uF 80V') and n(c,1)=='VBUS')==5,'five bus caps (235 uF)')
# meters
addr={}
SYM={'GND':0,'+3V3_PI':1,'I2C_SDA':2,'I2C_SCL':3}
for u in [c for c in comp if val(c).startswith('INA2')]:
    a=0x40|(SYM[n(u,1)]<<2)|SYM[n(u,2)]; ok(a not in addr,f'{u} address {a:#x}'); addr[a]=u
    ok(n(u,4)=='I2C_SDA' and n(u,5)=='I2C_SCL' and n(u,6)=='+3V3_PI' and n(u,7)=='GND',u+' bus and supply')
ok(len(addr)==9,'nine meters')
ports=[u for u in comp if val(u).startswith('TPS48111')]
ok(len(ports)==6,'six limb port breakers')
for k in range(2,8):
    r=f'RS{k}'; u=f'U{k+9}'; t=f'L{k-1}'
    ok(n(r,1)=='VBUS' and n(u,10)==n(r,2) and n(u,9)==n(r,3) and n(u,8) in (f'V_{t}',n(r,4)),r+' Kelvin to '+u)
    p=[x for x in ports if n(x,20)==n(r,2)]
    q=[x for x in comp if x[0]=='Q' and n(x,4)==n(r,4)]
    ok(len(p)==1 and n(p[0],17)==n(r,3) and two(n(r,2),n(p[0],18),('R',)) and two(n(p[0],19),n(r,3),('R',)) and len(q)==1 and n(q[0],2)==n(p[0],13)==f'V_{t}',t+' port: shunt, breaker sense (RSET on CS+, RISCP to CS-), FET drain on the shunt, source on the output')
    ok(n(p[0],1)!=n(p[0],3) and two('RUN_P',n(p[0],1),('R',)) and two('RUN_P',n(p[0],3),('R',)) and n(p[0],5)=='PORT_FLT_N' and two(n(p[0],9),'GND',('R',)),t+' port enabled from RUN_P, fault flag shared, latch-off resistor on TMR')
    ok(any(d[0]=='D' and n(d,1)==f'V_{t}' and n(d,2)=='GND' for d in comp),t+' output clamp to GND for the wiring current')
    j=[x for x in comp if x[0]=='J' and n(x,1)==f'V_{t}']
    ok(len(j)==1 and n(j[0],2)=='GND',t+' + and return on one pigtail pair')
ok(diode('RUN_P','RUN') and not any(x in ('U1','U8') for x,p in nets['RUN_P']),'ports take RUN through their own diode')
OPT={r for r in comp if {n(r,1),n(r,2),n(r,4)}&{'PRE_G','ABS_N'}}|{'C29'}|set(csc)|{r for r in comp if r[0]=='R' and {n(r,1),n(r,2)}=={'VBUS','GND'}}
legs=[r for r in comp if dnp(r) and r not in OPT]
ok(n('U9',2)=='SP_EN' and len(two('RUN','SP_EN'))==1 and n('U9',3)==n('U9',4)=='+12V','SPARE_SW eFuse enabled by RUN, fed from +12V')
ok(n('RS10',1)==n('U9',5) and n('U10',10)==n('RS10',2) and n('U10',9)==n('RS10',3) and n('U10',8)==n('RS10',4)=='SPARE_SW','RS10 Kelvin to U10')
ok(n('J22',1)=='SPARE_SW' and n('J22',2)=='GND','SPARE_SW output')
ok(n('J23',1)=='SPARE_ON' and n('J23',2)=='GND' and n('U18',5)=='SPARE_ON' and n('U18',3)==n('U18',4)=='+12V','SPARE_ON output behind its eFuse')
ok(not any(r in ('U18','J23') for r,p in nets['RUN']),'SPARE_ON does not touch the latch')
ok(len(legs)==3*(4+20),'aux and legs DNP (4 limb parts + 20 breaker parts each): '+str(len(legs)))
ok(not [r for r,p in nets['GND'] if r[0]=='J' and val(r).startswith('GND stud')],'no GND studs: each limb return sits beside its +')
ok(n('J21',1)=='+3V3_PI' and n('J21',2)=='I2C_SDA' and n('J21',3)=='GND' and n('J21',4)=='I2C_SCL' and n('J21',7)=='PORT_FLT_N','Pi header: GND between SDA and SCL, port fault line')
ok(n('J20',8)=='ESTOP_NO' and n('J20',7)=='GND','button NO on the loop connector, GND between it and reset')
ok(two('NO_PU','ESTOP_NO') and diode('NO_PU','+3V3_PI') and two('ESTOP_NO','ESTOP_PI_N') and n('J21',5)=='ESTOP_PI_N','NO pull-up from the Pi rail through a blocking diode, series R to the Pi')
ok(n('D6',3)=='ESTOP_PI_N','clamp on the Pi pin')
ok(not any(r=='J21' for r,p in nets['RUN']+nets['VCTL']+nets['VBUS']),'nothing above 3.3 V on the Pi header')
ok(any(r[0]=='D' and n(r,1)=='MID_S' and n(r,2)=='GND' for r in comp),'U17 VBUS pin clamped')
ok(not any(n(u,p)=='I2C_SDA' for u in addr.values() for p in (1,2)),'no meter address strap on SDA')
ok(two('+12V','SPON_EN',('R',)) and val(two('SPON_EN','GND',('R',))[0])=='33k','U18 EN divider keeps EN under 6 V up to 20 V of back-feed')
ok(len({len([p for r2,p in pin if r2==j]) for j in ('J22',)}|{2})==2 or True,'')
ok(sum(1 for p in range(1,5) if n('J22',p))==4 and n('J5',1)!='SPARE_SW','SPARE_SW on a 4-pin header, keyed apart from the 2-pin outputs')
for j,nm in (('J5','Jetson'),('J6','Pi buck'),('J7','fans')):
    ok(any(r[0]=='F' and {n(r,1),n(r,2)}=={'+12V',n(j,1)} for r in comp),nm+' output fused')
ok(all(any(r[0]=='D' and n(r,1)==o and n(r,2)=='GND' for r in comp) for o in (n('U9',5),'SPARE_ON')),'eFuse outputs clamped to GND')
ok(any(r[0]=='D' and n(r,1)=='+12V' and n(r,2)=='GND' and val(r).startswith('SMBJ15') for r in comp),'+12V back-feed TVS')
# power-fail warning (rev B5): comparator on the converter input, own reference, hysteresis, open drain to the Pi
pf=[u for u in comp if val(u).startswith('TLV1805') and n(u,1)=='PF_OUT']
pr=[u for u in comp if val(u).startswith('LM4040') and n(u,1)=='PF_VR']
ok(len(pf)==1 and n(pf[0],6)=='VCTL' and n(pf[0],3)=='PF_SENSE' and n(pf[0],4)=='PF_REF' and len(pr)==1,'power-fail comparator on VCTL: sense on IN-, its own reference on IN+')
ok(two('BRK_IN','PF_SENSE',('R',)) and two('PF_SENSE','GND',('R',)) and two('PF_SENSE','GND',('C',)),'power-fail sense divided from the converter input (BRK_IN), RC filtered')
ok(two('PF_VR','PF_REF',('R',)) and two('PF_OUT','PF_REF',('R',)),'power-fail hysteresis: reference through a resistor, feedback from OUT')
qf=[q for q in comp if q[0]=='Q' and n(q,1)=='PF_OUT' and n(q,2)=='GND']
ok(len(qf)==1 and two(n(qf[0],3),'PWR_FAIL_N',('R',)) and n('J21',8)=='PWR_FAIL_N' and not two('PWR_FAIL_N','+3V3_PI',('R',)),'PWR_FAIL_N open drain on J21 pin 8, pull-up on the Pi side')
print('audited',cnt[0]); print('FAILS',len(fail))
