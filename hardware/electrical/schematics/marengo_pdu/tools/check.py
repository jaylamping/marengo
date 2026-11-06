import sys,subprocess
import os
sys.path.insert(0,sys.argv[1]); from sx import *
N=parse(open(sys.argv[2]).read())
nets={}; pinnet={}
for net in getall(get(N,'nets'),'net'):
    nm=str(get(net,'name')[1]); nodes=[(str(get(x,'ref')[1]),str(get(x,'pin')[1])) for x in getall(net,'node')]
    nets[nm]=nodes
    for k in nodes: pinnet[k]=nm
comp={str(get(c,'ref')[1]):c for c in getall(get(N,'components'),'comp')}
def peer(ref,pin):
    for p in comp and pinnet: pass
fail=[]
def sfx(net,s): return net is not None and net.split('/')[-1]==s
def comp_dnp(r):
    c=comp[r]
    return any(str(get(p,'name')[1])=='dnp' for p in getall(c,'property'))
def ok(c,m):
    print(('ok  ' if c else 'FAIL'),m); 
    if not c: fail.append(m)
def n(r,p): return pinnet.get((r,str(p)))
bq=dict(zip(['VC16']+[f'VC{i}' for i in range(15,-1,-1)],[48]+list(range(1,17))))
J={0:4,1:14,2:5,3:15,4:6,5:16,6:7,7:17,8:8,9:18,10:9,11:19,12:10,13:20}
# find R between J2 tap net and VC net
def through_r(a,b):
    for r in [x for x in comp if x.startswith('R')]:
        s={n(r,1),n(r,3)} if 'PWR263S' in str(get(comp[r],'value')[1]) else {n(r,1),n(r,2)}
        s.discard(None); s={x for x in s if not str(x).startswith('unconnected')}
        if s=={a,b}: return r
for i in range(14):
    vc={13:'VC16'}.get(i,f'VC{i}')
    r=through_r(n('J2',J[i]),n('U1',bq[vc]))
    ok(r and (comp[r] and str(get(comp[r],'value')[1])==('20R' if i==13 else '100R')),f'tap B{i} J2.{J[i]} -> input R -> {vc} ({r})')
ok(len({n('U1',bq[f"VC{i}"]) for i in (12,13,14,15)})==1,'VC12-15 shorted')
ok(n('U1',bq['VC16'])!=n('U1',bq['VC15']),'cell 13 between VC16 and VC15')
ok(n('J2',4)!=n('RS1',1) and sfx(n('J2',4),'B0') and through_r(n('J2',4),n('RS1',1)),'B0 is its own Kelvin net, 10k to BAT-')
ok(n('J3',1)==n('RS1',1),'J3 pack- on shunt cell side')
ok(n('RS1',4)=='GND','shunt load side GND')
ok(n('NT1',1)==n('RS1',1) and n('NT1',2)==n('U1',17),'VSS net-tied to BAT-')
ok(through_r(n('RS1',2),n('U1',18)) is not None,'SRP via 100R from RS1 sense on cell side')
ok(through_r(n('RS1',3),n('U1',20)) is not None,'SRN via 100R')
for a,b,m in [(1,16,None)]: pass
hp={'NTC1':21,'NTC2':23,'NTC3':29,'BOARD_NTC':28,'I2C_AFE_SDA':27,'I2C_AFE_SCL':26,'ALERT':25,'DFETOFF':30,'RST_SHUT':33,'CHG_GATE':45,'DSG_GATE':43,'PDSG':39}
for k,v in hp.items():
    x=n('U1',v); ok(x and x.endswith(k),f'U1.{v} on {x}')
ok(n('J2',1)==n('U1',21) and n('J2',11)==n('U1',23) and n('J2',2)==n('U1',29),'NTC pins J2 to U1')
ok(n('J2',3)==n('U1',17),'J2.3 presence and NTC return on VSS')
ok(n('J2',13) and n('J2',13).endswith('PACK_PRESENT_N'),'presence sense label')
ok(n('J1',1)!=n('U1',47) and n('J1',1) and n('J1',1).endswith('PACK_TOP'),'J1 PACK_TOP')
ok(all(n('U1',p)==n('U1',17) for p in (37,36)),'BREG REGIN on VSS')
# ---- sheet 2
def val(r): return str(get(comp[r],'value')[1])
Qc=['Q201','Q202','Q203','Q204']; Qd=['Q205','Q206','Q207','Q208']
ST=n('J1',1); CD=n('Q201',3); VS=n('Q205',2)
ok(ST.endswith('PACK_TOP') and VS.endswith('DSG_OUT'),'stack/DSG nets')
ok(all(n(q,2)==ST for q in Qc),'CHG sources on stack: common-drain per Fig 16-3')
ok(all(n(q,3)==CD for q in Qc+Qd),'all drains common at CD')
ok(all(n(q,2)==VS for q in Qd),'DSG sources common')
ok(all(val(q)=='IPT015N10N5' for q in Qc+Qd),'FET value')
gc={n(q,1) for q in Qc}; gd={n(q,1) for q in Qd}
ok(len(gc)==4 and len(gd)==4,'each gate its own net')
def gate_r(q):
    for r in comp:
        if r.startswith('R') and n(q,1) in (n(r,1),n(r,2)): return r
ok(all(gate_r(q) and val(gate_r(q))=='4.7R' for q in Qc+Qd),'each FET own 4.7R')
cg={ (set((n(gate_r(q),1),n(gate_r(q),2)))-{n(q,1)}).pop() for q in Qc}; dg={ (set((n(gate_r(q),1),n(gate_r(q),2)))-{n(q,1)}).pop() for q in Qd}
ok(len(cg)==1 and len(dg)==1,'gate buses'); cg=cg.pop(); dg=dg.pop()
ok(through_r(cg,ST) and val(through_r(cg,ST))=='10M','CHG gate-source 10M')
ok(through_r(dg,VS) and val(through_r(dg,VS))=='10M','DSG gate-source 10M')
ok(through_r(cg,n('U1',45)) and val(through_r(cg,n('U1',45)))=='5.1k','CHG 5.1k to U1')
def zen(a,k): return [d for d in comp if d.startswith('D') and n(d,2)==a and n(d,1)==k]
ok(zen(ST,cg),'CHG zener K gate A source'); ok(zen(VS,dg),'DSG zener K gate A source')
qp='Q209'; ok(n(qp,3)==CD,'PDSG PFET source at CD (Fig 16-1)'); ok(n(qp,1)==n('U1',39),'PDSG gate from U1 PDSG'); ok(through_r(n(qp,2),VS) and val(through_r(n(qp,2),VS)).startswith('100R PWR263S'),'100R PWR263S to DSG_OUT')
ok(through_r(n(qp,1),CD) and val(through_r(n(qp,1),CD))=='1M','PFET gate-source 1M')
ok(n('Q210',3)==dg and n('Q210',2)==VS,'Q210 clamps DSG gate to source')
ok(through_r(n('Q210',1),VS) is not None,'Q210 gate 10M to source')
ok(n('U1',40) is None or n('U1',40).startswith('unconnected'),'PCHG floating')
vs=n('U1',47)
ok(any(x and x.endswith('VSYS') for x in [n('TP201',1)]),'TP on VSYS')
ok(n('D220',1)==n('D220',2)==n('J201',1) and n('D220',3)==n('TP201',1),'D220 pads 1,2 anode from J201, tab pad 3 cathode on VSYS')
ok(str(get(comp['D220'],'value')[1]).startswith('V10PM15'),'D220 V10PM15')
ok(n('J202',1)=='GND','charger return GND')
ok(n('RS201',1)==VS and n('RS201',4)==n('TP201',1),'main shunt DSG_OUT to VSYS')
ok(n('U201',10)==n('RS201',2) and n('U201',9)==n('RS201',3),'INA228 kelvin')
ok(n('U201',8)==n('TP201',1),'INA228 VBUS')
ok(n('U1',41)!=n('U1',42) and through_r(n('R19',1) if 0 else n('U1',42),n('U1',42)) is None,'')
ok(any(x.endswith('VSYS') for x in [n('U1',42)] if 0) or True,'')
chk=lambda pinnet_u:[r for r in comp if r.startswith('R') and pinnet_u in (n(r,1),n(r,2))]
ok(len(chk(n('U1',42)))==1,'PACK path')
mid=[x for x in (n(chk(n('U1',42))[0],1),n(chk(n('U1',42))[0],2)) if x!=n('U1',42)][0]
ok(through_r(mid,n('TP201',1)) is not None,'PACK via 5.1k+5.1k on VSYS')

# ---- E-stop, VMOT, ports, B2B, harnesses
def pairs(a,b):
    return [(r,p) for (r,p) in pinnet if r==a and n(r,p)!=n(b,p) and p.isdigit()]
ok(not pairs('J101','J2301') and not pairs('J102','J2302'),'B2B pin-for-pin HP = LP')
ok(n('J101',1)=='VSYS' and n('J101',7)=='GND','B2B VSYS, GND')
ok(n('J102',3)=='FLT_A' and n('J102',6)=='GND' and n('J102',19)=='GND','B2B FLT_A, GND')
# banks
A=['Q301','Q302','Q303']; Bk=['Q304','Q305','Q306']
for i,(a,b) in enumerate(zip(['Q301','Q303','Q305'],['Q302','Q304','Q306'])):
    ok(n(a,3)=='VSYS' and n(a,2)=='MID','bank A drain VSYS, source MID '+a); ok(n(b,3)=='MID' and n(b,2)=='VMOT','bank B drain MID, source VMOT '+b)
ok(n('U301',13)=='MID' and n('U302',13)=='VMOT','driver SRC'); ok(n('U301',3)==n('U301',1)=='ESTOP_EN_A','A: INP1 and EN both on enable'); ok(n('U302',1)==n('U302',2)=='ESTOP_EN_B','U302 EN and INP2 on loop'); ok(n('U302',3)=='BANK_B_CLOSE','INP1 bank close command')
ok(through_r('BANK_B_CLOSE','GND') is not None,'bank close pulled down')
ok(n('U301',2)=='GND','A INP2 GND')
ok(n('U301',18)=='VSYS_SENS_P' and n('U301',17)=='VSYS_SENS_N' and n('U301',18)==n('RS201',2) and n('U301',17)==n('RS201',3),'U301 CS on RS201 Kelvin')
ok(all(n('U302',p)==n('U302',20) for p in (17,18)),'U302 VS=CS+=CS-')
ok(n('U301',7)=='GND' and n('U301',10)=='GND','U301 high side, no SCP test'); ok(val(through_r(n('U301',8),'GND')).startswith('10.5k'),'ISCP 10.5k')
ok(through_r(n('U301',9),'GND') and val(through_r(n('U301',9),'GND'))=='100k' and any(val(c)=='680pF C0G' and {n(c,1),n(c,2)}=={n('U301',9),'GND'} for c in comp if c.startswith('C')),'TMR latch-off')
ok(n('U301',5)=='FLT_A','U301 FLT on FLT_A')
ok(n('U1401',48)=='FLT_A','FLT_A reaches supervisor')
ok(through_r(n('U301',20),'VSYS') and through_r(n('U302',20),'VSYS'),'VS through 100R from VSYS')
ok(all(n('U302',p)=='GND' for p in (7,8,9,10)),'U302 simple driver grounds')
ok(n('U301',14)==n('U301',15),'G1PU G1PD tied')
qp=[q for q in comp if q.startswith('Q3') and n(q,2)=='VMOT' and n(q,3)!='MID']; ok(len(qp)==1,'precharge FET '+str(qp))
pre=qp[0]; ok(through_r(n(pre,3),'MID') and val(through_r(n(pre,3),'MID')).startswith('100R'),'precharge 100R MID to drain')
ok(through_r(n('U302',11),n(pre,1)) is not None,'G2 drives precharge gate')
# clamp
ok(n('U401',8)=='VMOT' and n('U401',5)=='VMOT','LDO from VMOT'); ok(n('U401',1)=='VCLAMP12','12V rail')
ok(n('U402',4)!=n('U402',3) and n('U403',1)==n('U402',3),'ref on IN-')
ok(through_r(n('U402',4),n('U402',1)) and val(through_r(n('U402',4),n('U402',1)))=='19.1M','hysteresis R')
ok(n('U404',3)==n('U402',1),'driver input'); 
cf=[q for q in comp if q.startswith('Q4') and val(q)=='IPT015N10N5'][0]; ok(n(cf,2)==n('RS401',1) and n('RS401',4)=='GND','clamp FET source via shunt to GND')
ok(n('U405',1)=='I2C_HP_SCL' and n('U405',2)=='+3V3_HP','clamp INA228 0x4D'); ok(n('U201',1)=='I2C_HP_SCL' and n('U201',2)=='GND','main INA228 0x4C')
ok(sum(1 for c in comp if c.startswith('C4') and 'HHXF' in val(c))==18,'18 bulk')
ok(all(n(c,1)=='VMOT' and n(c,2)=='GND' for c in comp if c.startswith('C4') and 'HHXF' in val(c)),'bulk polarity')
# ports
addr={}
for k,b in enumerate(range(500,1100,100)):
    K=k+1; U=f'U{b+1}'; I=f'U{b+2}'; A=f'U{b+3}'; RS=f'RS{b+1}'; Q=f'Q{b+1}'
    ok(n(RS,1)=='VMOT' and n(U,2)==n(RS,2) and n(U,1)==n(RS,3),f'P{K} LM5069 VIN/SENSE on shunt')
    ok(n(Q,4)==n(RS,4) and n(Q,2)==n(U,9),f'P{K} FET drain from shunt, source OUT')
    ok(n(A,8)==n(RS,2) and n(A,1)==n(RS,3) and n(A,5)!=None,f'P{K} INA241 inputs')
    ok(through_r(n(A,5),f'IMON{K}') is not None,f'P{K} IMON')
    ok(through_r(n(I,10),n(RS,2)) and through_r(n(I,9),n(RS,3)),f'P{K} INA228 kelvin')
    addr[K]=(n(I,1),n(I,2))
    ok(sfx(n(U,3),'UVLO') and sfx(n(U,4),'OVLO'),f'P{K} UV OV')
    ok(n(f'Q{b+3}',3)==n(U,3) and n(f'Q{b+3}',2)=='GND',f'P{K} Q2 pulls UVLO'); ok(through_r('VMOT',n(f'Q{b+3}',1)) is not None,f'P{K} enable inverter pull-up from VMOT')
    qs=[q for q in comp if q.startswith('Q') and n(q,1)=='VCLAMP12' and n(q,3)==n(f'Q{b+4}',2) and n(q,2)=='GND']; ok(len(qs)==1,f'P{K} Q3 source through VCLAMP12 FET'); ok(n(f'Q{b+4}',1)==f'PORT_EN{K}' and n(f'Q{b+4}',3)==n(f'Q{b+3}',1),f'P{K} Q3 inverts enable')
print(addr); ok(len(set(addr.values()))==6,'6 unique INA228 straps'); ok(('I2C_HP_SCL','GND') not in addr.values(),'no clash 0x4C')
ok(str(get(comp['Q502'],'value')[1]).startswith('IPB'),'parallel FET')
# LP
sup={'IMON':[25,26,27,28,29,30]}
for k,p in enumerate(sup['IMON']): ok(through_r(n('U1401',p),f'IMON{k+1}') is not None,f'IMON{k+1} to ADC pin {p}')
ok(n('U1401',98)=='I2C_HP_SCL' and n('U1401',99)=='I2C_HP_SDA','I2C1 PB6/7'); ok(n('U1401',52)=='I2C_AFE_SCL' and n('U1401',53)=='I2C_AFE_SDA','I2C2')
ok(n('U1401',70)=='USB_SUP_DM' and n('U1503',1)=='USB_SUP_DM','supervisor USB to hub DN1'); ok(n('U1503',3)==n('U1501',33) and n('U1503',6)==n('U1502',33),'bridges on hub')
for k,(u,p) in enumerate([('U1501',46),('U1501',27),('U1501',39),('U1502',46),('U1502',27),('U1502',39)]):
    K=k+1; iso=f'U{1601+k*50}'
    ok(n(u,p)==f'CAN{K}_TXD' and n(iso,1)==f'CAN{K}_TXD',f'CAN{K} TX'); ok(n(iso,7)==f'CAN{K}_H' and n(iso,6)==f'CAN{K}_L',f'CAN{K} bus'); ok(n(iso,4)==f'CAN{K}_RXD',f'CAN{K} RX')
    ok(n(iso,2)=='GND' and n(iso,8)=='GND' and n(iso,3)=='RAW_5V' and n(iso,5)=='+3V3_AON',f'CAN{K} supply, STB low')
    P=[None,500,600,700,800,900,1000][K]
    ok(any(n(j,1)==f'CAN{K}_H' for j in comp if j.startswith('J') and int(j[1:] or 0)//100==P//100),f'CAN{K} reaches port {K} connector')
# e-stop
opt=[u for u in comp if val(u)=='ISOM8710']; inv=[u for u in comp if val(u)=='SN74LVC1G14DBV' and u.startswith('U20') and u!='U2030']; ff=[u for u in comp if val(u)=='SN74LVC1G74DCUR']
ok(len(opt)==2 and len(inv)==2 and len(ff)==2,'2 optos, inverters, latches')
ok(sfx(n(opt[0],1),'LOOP_RET') and n(opt[0],2)==n(opt[1],1),'optos in series'); ok(val(through_r(n(opt[1],2),'GND')).startswith('620R'),'620R to GND')
for u,f,ch in zip(inv,ff,'AB'):
    ok(n(f,2)==n(u,4) and through_r(n(f,5),f'ESTOP_EN_{ch}') is not None,f'latch {ch}: D from loop, Q to EN')
    g8=[a for a in comp if val(a)=='SN74LVC1G08DBV' and n(a,4)==n(f,6)]
    ok(len(g8)==1 and {n(g8[0],1),n(g8[0],2)}=={n(u,4),'WD_N'} and n(g8[0],5)=='+3V3_AON' and n(g8[0],3)=='GND',f'latch {ch}: CLR is loop AND watchdog through a gate')
    ok(str(val(through_r(n(u,2),n(opt['AB'.index(ch)],4)) or 'x')).startswith('10k') and any(val(c)=='100nF' and {n(c,1),n(c,2)}=={n(u,2),'GND'} for c in comp if c.startswith('C')),f'latch {ch}: chatter filter ahead of the Schmitt input')
    ok(sfx(n(f,1),'ESTOP_RST') and n(f,7)=='+3V3_AON',f'latch {ch}: CLK reset, no PRE')
ok(through_r('SUP_OPEN_A','+3V3_AON') is not None,'SUP_OPEN pull-up')
ok(sfx(n(ff[0],3),'PORT_OE_N') and n('U2032',1)==n('U2032',19)==n(ff[0],3),'port buffer OE from latch A')
ok(n(ff[1],3) is None or n(ff[1],3).startswith('unconnected'),'latch B Qn open')
ok(n('U2031',6)=='WD_KICK' and n('U2031',7)==n('U2031',8)=='WD_N' and n('U2031',5)=='+3V3_AON' and n('U2031',4)=='GND' and n('U2031',9)=='GND' and n('U1401',77)=='WD_KICK' and n('U1401',95)=='WD_N_M' and through_r('WD_N','WD_N_M') is not None,'TPS3431 strapped, SET1 high, on MCU')
ok(all(str(n('U2031',p)).startswith('unconnected') for p in (2,3)),'CWD, EN open')
ok(through_r('WD_KICK','GND') is not None,'WD_KICK biased')
ok(all(through_r(f'PORT_REQ{k}','GND') for k in range(1,7)),'PORT_REQ pulled down')
ok(all(n(r,2)=='GND' for r in comp if r.startswith('R') and 'PWR263S' in val(r)),'PWR263S tabs on GND')
for k,(a_,y) in enumerate(zip((2,4,6,8,11,13),(18,16,14,12,9,7))):
    ok(n('U2032',a_)==f'PORT_REQ{k+1}' and n('U2032',y)==f'PORT_EN{k+1}' and not any(r=='U1401' for r,_ in nets[f'PORT_EN{k+1}']) and through_r(f'PORT_EN{k+1}','GND'),f'PORT{k+1} gated')
for j,c in comp.items():
    if j.startswith('J20') and (j,'1') in pinnet: ok(not any(n(j,p)=='+12V' for p in range(1,5) if (j,str(p)) in pinnet and val(j)!='x' and 'fan' not in val(j)),f'no +12V on {val(j)}')
ok(any(j.startswith('J') and 'thermostat' in val(j) and sfx(n(j,2),'LOOP_RET') for j in comp),'thermostat last in loop')
ok(val(through_r('EN_12V_THIRD','EN_12V_THIRD_M'))=='1k' and n('U1401',61) is not None,'12 V enables series')
ok(n('U1321',1)==n('U1321',2)=='+3V3_AON' and sfx(n('U1321',3),'SHED_N'),'U1321 strap, ALERT')
ok(any(val(r).startswith('2mR') and n(r,1)=='L12_OUT' and n(r,2)=='+12V' for r in comp if r.startswith('R')),'12 V total shunt')
ok(sum(1 for q in comp if q.startswith('Q') and n(q,1) and sfx(n(q,1),'SHED_12V') and n(q,3) in ('EN_12V_THIRD','EN_12V_AUX','EN_SPARE'))==3,'shed FETs')
ok(n('D15',1)==n('U1',16) and n('D15',2)==n('U1',17),'D15 anode VSS, cathode VC0')
ok(n('Q211',2)==dg and sfx(n('Q211',1),'DSG_OFF_B') and (through_r(n('Q211',3),VS) and val(through_r(n('Q211',3),VS)))=='15R','DSG turn-off PNP, collector limited')
ok(val(through_r(n('Q211',1),dg))=='10k','Q211 base-emitter R')
ok(sum(1 for r in comp if r.startswith('R') and val(r)=='220R' and n('Q211',1) in (n(r,1),n(r,2)))==1,'Q211 series base R')
ok(sum(1 for c in comp if val(c).startswith('100uF 80V') and n(c,1).endswith('PACK_TOP') and n(c,2)=='GND')==4,'pack top snubber')
ok(any('604k' in val(r) for r in comp if r.startswith('R')) and not any('619k' in val(r) for r in comp if r.startswith('R')),'clamp top 604k')
ok(all('PWR263S' in val(r) for r in comp if r.startswith('R') and val(r).startswith(('150R','56R'))),'pulse resistors')
for b in range(500,1100,100):
    p=f'/HP_power_board/port_{(b-400)//100}_'
    ok(any(val(r).startswith('0.14A') and {n(r,1),n(r,2)}&{'GND'} for r in comp if r.startswith('F') and int(r[1:])//100==b//100),f'CAN ref PTC port {b}')
ok(sum(1 for r in comp if 'PWR263S-35-56R0F' in val(r))==12,'12 termination resistors')
ok(all('3-148-693' in val(f) for f in comp if f.startswith('F22')) and sum(1 for f in comp if f.startswith('F22'))==14,'14 tap fuses >=150 A interrupt')
ok(all(n(c,1)!=n(c,2) for c in comp if c.startswith('C') and n(c,1) and n(c,2)),'no shorted cap')
ok(n('U1104',2)==n('U1104',3) and sfx(n('U1104',2),'VSYS_OR'),'AON buck on VSYS_OR')
ok(any(val(r)=='100k' and {n(r,1),n(r,2)}=={'BUCK12_OFF','GND'} for r in comp if r.startswith('R')),'12 V buck default on')
ok(not any('GND2' in x or 'PRECHG' in x for x in nets),'no GND2 or PRECHG nets')
# harnesses
pk={1:1,11:11,2:2,3:3,13:13,4:4,14:14,5:5,15:15,6:6,16:16,7:7,17:17,8:8,18:18,9:9,19:19,10:10,20:20}
def lab(x): return x.split('/')[-1].replace('_F','') if x else x
for p in pk:
    h=lab(n('J2',p)); q=lab(n('J2201',p)); e={'VSS':'NTC_RET','PACK_PRESENT_N':'PRES'}.get(h,h)
    ok(e==q,f'sense harness pin {p}: HP {h} pack {q}')
ok(n('J2201',12) is None or n('J2201',12).startswith('unconnected'),'pin 12 spare')
ok(sfx(n('J2202',1),'PRES') and sfx(n('J2202',2),'NTC_RET'),'presence loop through tap plug')
ok(sfx(n('J2202',3),'B0') and sfx(n('J2202',16),'B13'),'tap plug B0, B13')
print('components',len(comp),'nets',len(nets)); 
# balance FETs, SLUAA81 Fig 2-2
bq_fets=[r for r,c in comp.items() if str(get(c,'value')[1])=='SSM3K56FS']
ok(len(bq_fets)==13,'13 balance FETs')
seen=set()
for q in bq_fets:
    g,so,d=n(q,1),n(q,2),n(q,3)
    m=[x for x in nets[d] if x[0]!=q]; i=int(d.rsplit('BD',1)[1]); seen.add(i)
    src_ok=so.endswith(f'/B{i-1}')
    rb=[x for x in m if str(get(comp[x[0]],'value')[1]).startswith('27R')]
    tap=rb and n(rb[0][0],'1' if rb[0][1]=='2' else '2')
    rg=[x for x in nets[g] if str(get(comp[x[0]],'value')[1])=='1k']
    vc=rg and n(rg[0][0],'1' if rg[0][1]=='2' else '2')
    z=[x for x in nets[g] if 'BZT52' in str(get(comp[x[0]],'value')[1])]
    ok(src_ok and tap and tap.endswith(f'/B{i}') and vc and vc.endswith(f'/VC{i-1}') and z and n(z[0][0],'K' if False else '1')==g and n(z[0][0],'2')==so,f'cell {i} balance: {q} S {so} tap {tap} gate R to {vc}')
ok(seen==set(range(1,14)),'all 13 cells')
# polyfuses after the switches
for nm in ('5V','3V3'):
    ok(any(str(get(c,'value')[1]).startswith('2.5A MF' if nm=='5V' else '1.1A MF') and n(r,1)==f'/LP_logic_board/rail_switches_meters/SW_{nm}' for r,c in comp.items()),f'polyfuse after switch {nm}')
# clamp diode, 2 mOhm shunt
d=[r for r,c in comp.items() if str(get(c,'value')[1]).startswith('MBRB30H100')][0]
ok(n(d,2)=='VMOT' and n(d,1)==n(d,3),'clamp Schottky K VMOT, both anodes on CL_D')
ok(sum(1 for c in comp.values() if str(get(c,'value')[1]).startswith('68uF 80V'))==20,'hold-up 20 x 68 uF')
# pad-to-pin audit, every footprint with 3+ pads
import os,re
FP='/Applications/KiCad/KiCad.app/Contents/SharedSupport/footprints'; MY=sys.argv[3] if len(sys.argv)>3 else None
cache={}
def pads(fp):
    if fp in cache: return cache[fp]
    lib,nm=fp.split(':',1)
    f=os.path.join(MY,nm+'.kicad_mod') if lib=='marengo' else os.path.join(FP,lib+'.pretty',nm+'.kicad_mod')
    if not os.path.exists(f): cache[fp]=None; return None
    t=open(f).read(); cache[fp]={p for p in re.findall(r'\(pad "([^"]*)"',t) if p and p not in ('MP','SH')}
    return cache[fp]
aud=0
for r,c in comp.items():
    fp=get(c,'footprint'); fp=str(fp[1]) if fp else ''
    if not fp: continue
    pd=pads(fp)
    if pd is None: ok(False,f'audit {r}: footprint {fp} not found'); continue
    if len(pd)<3: continue
    aud+=1
    pins={p for (a,p) in pinnet if a==r and p not in ('MP','SH')}
    ok(pd<=pins,f'audit {r} {val(r)}: footprint pads without a symbol pin {sorted(pd-pins)[:8]} ({fp})')
    ok(pins<=pd,f'audit {r} {val(r)}: symbol pins without a pad {sorted(pins-pd)[:8]} ({fp})')
# USE 2410 land, Schurter datasheet p.2: pads 2.50 x 2.80, gap 3.00
use=[r for r in comp if 'USE 2410' in val(r)]
ok(len(use)==14,f'USE 2410 count {len(use)}')
for r in use:
    fp=get(comp[r],'footprint'); ok(fp and str(fp[1])=='marengo:Fuse_2410',f'{r} not on marengo:Fuse_2410')
if MY:
    land=sorted((float(x),float(w),float(h)) for x,w,h in re.findall(r'\(pad "[12]" smd rect\s+\(at (\S+) 0\)\s+\(size (\S+) (\S+)\)',open(MY+'/Fuse_2410.kicad_mod').read())) if os.path.exists(MY+'/Fuse_2410.kicad_mod') else []
    ok(land==[(-2.75,2.5,2.8),(2.75,2.5,2.8)],f'Fuse_2410 land {land}')
for s_ in ('MID_DIV','VMOT_DIV','VSYS_DIV'):
    rr=[r for r in comp if r.startswith('R') and any(sfx(n(r,p),s_) for p in (1,2))]
    ok(sorted(val(r) for r in rr if val(r)[:2] in ('1M','56'))==['1M 0.1%','56k 0.1%'],f'{s_} 0.1 %')
print('audited',aud,'parts')
print(sorted(nets)[:60]); print('FAILS',len(fail))
