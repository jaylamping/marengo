"""Rev B power board, one flat sheet. Edit this, not the .kicad_sch. Run tools/regen.sh."""
import os,sys,textwrap
sys.path.insert(0,os.path.dirname(os.path.abspath(__file__)))
from kit import *
wfile(OUT+'/sym-lib-table').write('(sym_lib_table\n\t(version 7)\n\t(lib (name "marengo_revb")(type "KiCad")(uri "${KIPRJMOD}/marengo_revb.kicad_sym")(options "")(descr "rev B project symbols"))\n)\n')
wfile(OUT+'/fp-lib-table').write('(fp_lib_table\n\t(version 7)\n\t(lib (name "marengo_revb")(type "KiCad")(uri "${KIPRJMOD}/marengo_revb.pretty")(options "")(descr "rev B project footprints"))\n)\n')
REDCUBE='TerminalBlock_Wuerth:Wuerth_REDCUBE-THR_WP-THRSH_74651194_THR'
TPFP='TestPoint:TestPoint_Keystone_5015_Micro_Mini'
GH=lambda n:f'Connector_JST:JST_GH_SM0{n}B-GHS-TB_1x0{n}-1MP_P1.25mm_Horizontal'
MF2='Connector_Molex:Molex_Micro-Fit_3.0_43045-0212_2x01_P3.00mm_Vertical'
SH='Resistor_SMD:R_Shunt_Isabellenhuette_BVR4026'
CP='Capacitor_SMD:CP_Elec_10x14.3'
CAP100='100uF 80V HHXF800GRA101MJH0G'
s=Sheet('root','marengo power board rev B','A1',comment='Rev B: a part goes in only if leaving it out would hurt. Parts list: hardware/electrical/pdb/REV_B_PARTS.md. Rev A stays as reference.')
s.file='marengo_pdu_revb.kicad_sch'; s.refc['Q']=2
def dnp(ref):
    for it in s.items:
        if it[0]=='symbol' and it[-1]==['__inst__',ref]:
            for x in it:
                if isinstance(x,list) and x and x[0]=='dnp': x[1]='yes'
            return
    raise KeyError(ref)
def conn(n_,val,at,fp,nets,ref=None):
    r=ref or s.nref('J'); nic(s,f'Connector_Generic:Conn_01x{n_:02d}',r,val,at,fp,{str(i+1):v for i,v in enumerate(nets)}); return r
def term(val,at,net): return conn(1,val,at,REDCUBE,[net])
def tp(g,net):
    p=g.at(); nic(s,'Connector:TestPoint',s.nref('TP'),'TP '+net,p,TPFP,{'1':net})
def head(t,at): s.text(t,at,size=2,bold=True)
def note(t,at): s.text(t,at,size=1.27)
def wnote(t,at,w): note('\n'.join(textwrap.fill(l,w) for l in t.split('\n')),at)
def Q(lid,val,at,b,e,c): r=s.nref('Q'); nic(s,lid,r,val,at,SOT23,{'1':b,'2':e,'3':c}); return r
NPN='Transistor_BJT:Q_NPN_BEC'; PNP='Transistor_BJT:Q_PNP_BEC'
mkic('IPB017N10N5LF',[('1','G','input'),('2','S','passive'),('3','S','passive'),('4','D','passive'),('5','S','passive'),('6','S','passive'),('7','S','passive')],'Package_TO_SOT_SMD:TO-263-7_TabPin4','https://www.infineon.com/dgdl/Infineon-IPB017N10N5LF-DataSheet-v02_02-EN.pdf','100 V 1.7 mOhm OptiMOS linear-mode FET, D2PAK-7',ref='Q')
mkic('IQB150W8-12',[('1','+VIN','power_in'),('2','R.C','passive'),('3','-VIN','power_in'),('5','RS-','input'),('6','TRIM','input'),('7','RS+','input'),('8','+VOUT','passive'),('4','-VOUT','passive')],'marengo_revb:IQB150W8','https://www.meanwell.com/Upload/PDF/IQB150W8/IQB150W8-SPEC.PDF','Mean Well 150 W quarter brick, 9-75 V in, 12 V 12.5 A out')
TLV=custom('TLV1805','tlv1805',ds='https://www.ti.com/lit/ds/symlink/tlv1805-q1.pdf')
LDO=custom('TPS7A4001','tps7a4001',ds='https://www.ti.com/lit/ds/symlink/tps7a40.pdf',renum={'EP':'9'})

# ---------- battery in and E-stop bank ----------
head('1. Battery in and E-stop bank B (LM5069-1, two FETs). Bank A, in series ahead of it, is block 7',(25.4,22))
term('M4 terminal, BMS P+',(40.64,45.72),'BAT')
term('M4 terminal, BMS P-',(40.64,68.58),'GND')
g=Grid(s,68.58,38.1,20.32,30.48,150)
g.Z('SMDJ58A','BAT','GND',SMC); g.C('1uF 100V','BAT','GND',C1210); g.C('100nF 100V','SH_P','GND',C805); g.F('BAT'); g.F('GND'); g.F('SH_P')
nic(s,'Device:R_Shunt','RS1','0.5mR CSS4J-4026R-L500F 5W',(177.8,48.26),SH,{'1':'BAT','2':'SH_P','3':'SH_N','4':'SW_D'})
Qb='marengo_revb:IPB017N10N5LF'
nic(s,Qb,'Q1','IPB017N10N5LFATMA1',(228.6,48.26),'Package_TO_SOT_SMD:TO-263-7_TabPin4',{'1':'G1','2':'VBUS','3':'VBUS','4':'MID','5':'VBUS','6':'VBUS','7':'VBUS'})
nic(s,Qb,'Q2','IPB017N10N5LFATMA1',(292.1,48.26),'Package_TO_SOT_SMD:TO-263-7_TabPin4',{'1':'G2','2':'VBUS','3':'VBUS','4':'MID','5':'VBUS','6':'VBUS','7':'VBUS'})
g=Grid(s,335.28,38.1,20.32,30.48,420)
g.R('10R','GATE','G1'); g.R('10R','GATE','G2')
nic(s,'Power_Management:LM5069MM-1','U1','LM5069MMX-1',(63.5,114.3),'Package_SO:MSOP-10_3x3mm_P0.5mm',{'1':'SH_N','2':'SH_P','3':'UVLO','4':'GND','5':'GND','6':'TIMER','7':'PWR','8':None,'9':'VBUS','10':'GATE'},ds='https://www.ti.com/lit/ds/symlink/lm5069.pdf')
g=Grid(s,114.3,96.52,20.32,30.48,420)
g.R('34k 1%','PWR','GND'); g.C('10nF C0G','TIMER','GND',C603); g.D('BAV19W','CDV','GATE'); g.C('100nF 100V C0G','CDV','GND',C1210); g.R('1k','GATE','CDV_B')
g.R('33k','RUN','UVLO'); g.R('33k','UVLO','GND')
Q(PNP,'MMBTA92',(275,111.76),'CDV_B','CDV','GND')
note('''The bank is the soft start for all the motor capacitance: BMS sees no inrush. Reset closes it, loop open drops it. No software in the path. Compute is not behind it.
Rs 0.5 mOhm: limit 97-123 A (VCL 48.5-61.5 mV), breaker 160-260 A (VCB 80-130 mV). Above peak draw and JK DOC 80 A. 1.8 W at 60 A.
Start: Cdv 100 nF, gate 10-22 uA, 100-220 V/s, 0.25-0.55 s to full. Bus ceiling 9.4 mF at 2.06 A hot DC SOA line (112 W, 45 K Tj rise). Drive caps unmeasured.
Power limit 652 W at 54.6 V, 480-810 W with tolerance: RPWR = 1.3e5 x Rs x (P - 1.18 mV x VDS / Rs), SNVS452G eq 10, VSNS 6 mV over the 5 mV floor. Timer 10 nF: 0.47 ms typ, 0.31-0.82 ms.
Fault check: 810 W is 14.8 A at 54.6 V for up to 0.82 ms, against about 22 A at 0.82 ms, 70 C, one FET, scaled from the 10 ms curve as sqrt(t): 1.5 x. Read the real curve at layout.
FETs: 0.85 mOhm for the pair, 5.2 W hot at 60 A, 1.3 W at 30 A, the same again in bank A. UVLO low sinks the gate at 2 mA: about 170 us off.
The 2 mA also pulls VBUS down through U1's OUT-GATE zener, against the 54 uA of its internal 1 MOhm SENSE-OUT path, so VBUS reads near 0 V with the banks open.
Body diodes of both banks pass regen back to the pack with the banks open. OVLO tied low: opening on overvoltage would strand regen on the bus.
LM5069-1 latches on a fault. Clear: E-stop cycle, then reset.
SMDJ58A: catches the battery lead's inductive kick when the bank opens. Clears 54.6 V, FETs and U1 are 100 V.''',(300,92))

# ---------- control rail and latch ----------
head('2. Control rail and E-stop latch (no logic IC)',(25.4,172))
nic(s,LDO,'U2','TPS7A4001DGNR',(63.5,200),'Package_SO:HVSSOP-8-1EP_3x3mm_P0.65mm_EP1.57x1.89mm',{'1':'VCTL','2':'FB_CTL','3':None,'4':'GND','5':'BAT','6':None,'7':None,'8':'BAT','9':'GND'})
g=Grid(s,114.3,185.42,20.32,30.48,200)
g.C('4.7uF 25V','VCTL','GND',C805); g.R('464k','VCTL','FB_CTL'); g.R('49.9k','FB_CTL','GND'); tp(g,'VCTL')
note('12.1 V, 11 mA. On the battery side, so regen through the body diodes keeps it up with the BMS open. Runs the latch and the clamp. 0.5 W at 54.6 V.',(114.3,232))
conn(7,'E-stop: 1 NC out, 2 GND, 3 NC return, 4-5 reset NO, 6 GND, 7 button NO',(241.3,200),GH(7),['LOOP_OUT','GND','LOOP_RET','RST_SRC','RST_IN','GND','ESTOP_NO'],ref='J20')
g=Grid(s,284.48,185.42,20.32,30.48,520)
g.R('680R 0.25W','VCTL','LOOP_OUT',R1206); g.Z('SMAJ15A','LOOP_RET','GND','Diode_SMD:D_SMA'); g.R('4.7k','VCTL','RST_SRC'); g.R('1M','RST_IN','GND'); g.C('2.2nF C0G','RST_IN','RST_C',C603); g.R('1k','RST_C','L_B')
Q(PNP,'MMBT3906',(419.1,205.74),'L_PB','LOOP_RET','RUN')
Q(NPN,'MMBT3904',(469.9,205.74),'L_B','GND','L_PC')
g=Grid(s,505,185.42,20.32,30.48,630)
g.R('100k','LOOP_RET','L_PB'); g.R('10k','L_PB','L_PC'); g.R('1.2k','RUN','GND'); g.R('10k','RUN','L_B'); g.R('10k','L_B','GND')
g.nl(); g.C('10nF','L_B','GND'); tp(g,'RUN')
note('''Seal-in latch, ngspice checked at hFE 80: stays off at power-up even with reset held, reset closes it, a 50 us contact bounce rides through,
loop open drops it in about 100 us, re-closing the loop does nothing, a reset held through a loop cycle does nothing, a new press works 5 ms after release.
RUN 7 V (UVLO 3.5 V): needs 5.7-6.1 V rising, so it holds. Loses VCTL, RUN drops, UVLO drops: bank opens.
Loop 6 mA, wetting for a control-rated NC contact. No 12 V on the connector without a series R, pin 1 next to GND: a short there opens the loop.
Brake resistor NC thermostat: wire it in series in the loop harness, no board parts.
Button HB2-BS545, 22 mm mushroom, twist release, 1NC + 1NO, 660 V AC 10 A, control-rated only: NC in this loop, NO to the Pi (block 9). RUN also enables bank A (block 7) and SPARE_SW (block 8).''',(284.48,240))

# ---------- clamp ----------
head('3. Braking clamp: 56.8 V on, 55.8 V off, 57.5 V worst case, into the HS25',(25.4,262))
nic(s,TLV,'U3','TLV1805DBVR',(63.5,292),'Package_TO_SOT_SMD:SOT-23-6',{'1':'CL_OUT','2':'GND','3':'VREF_CL','4':'CL_SENSE','5':'GND','6':'VCTL'})
nic(s,'Reference_Voltage:LM4040DBZ-2.5','U4','LM4040AIM3-2.5',(127,292),SOT23,{'1':'VREF_CL','2':'GND','3':None})
g=Grid(s,152.4,277,20.32,30.48,290)
g.R('9.09k','VCTL','VREF_CL'); g.C('100nF','VCTL','GND'); g.R('499k 0.1%','VBUS','CL_DIV',R1206); g.R('604k 0.1%','CL_DIV','CL_SENSE',R1206); g.R('51.1k 0.1%','CL_SENSE','GND'); g.R('19.1M','CL_OUT','CL_SENSE'); g.C('100pF C0G','CL_SENSE','GND')
g.nl(); g.R('10R','CL_OUT','CL_G'); g.R('100k','CL_G','GND'); g.D('MBRS3100T3G','VBUS','CL_D',SMC)
nfet5(s,'Q6','BSC040N10NS5ATMA1',(254,322),'CL_D','CL_G','GND')
term('M4 terminal, HS25 +',(330.2,292),'VBUS'); term('M4 terminal, HS25 -',(330.2,314.96),'CL_D')
note('''Arcol HS25 10 Ohm on the frame plate: 322 W, 5.7 A. Threshold values are rev A's, unchanged.
Self-powered: VCTL lives as long as the bus or the pack does. If the FET sticks on, the plate thermostat opens the loop and the bank drops the bus.
TLV1805 drives the FET directly: 100 mA peak, 0.3 us into 4 nF. Rev A's driver, meter, shunt, status FET and NTC are gone, and a smaller FET replaces the IPT015.
Leg stage later: fit 2 Ohm and recheck this FET, 29 A gives about 5 W.''',(355.6,280))

# ---------- bus caps ----------
head('4. Bus capacitors',(355.6,308))
g=Grid(s,368.3,330,38.1,30.48,560)
for _ in range(4): g.CP(CAP100,'VBUS','GND',CP)
g.F('VBUS'); tp(g,'VBUS')
note('400 uF, 80 V. The drives have their own. Can is 10 x 16.5: land owed, and it only clears under the higher ceiling, away from the fans.',(355.6,318))

# ---------- brick ----------
head('5. 12 V brick',(25.4,352))
nic(s,'marengo_revb:IQB150W8-12','U5','IQB150W8-12',(66,385),'marengo_revb:IQB150W8',{'1':'BRK_IN','2':'BRK_PG','3':'GND','5':'GND','6':None,'7':'+12V_RAW','8':'+12V_RAW','4':'GND'})
g=Grid(s,114.3,367,20.32,30.48,200)
g.C('2.2uF 100V','BRK_IN','GND',C1210,k=2); g.C('22uF 25V','+12V_RAW','GND',C1210,k=2); g.F('+12V_RAW')
nic(s,'Device:R_Shunt','RS8','2mR CSS4J-4026K-2L00F 6W',(218,378),SH,{'1':'+12V_RAW','2':'SH12_P','3':'SH12_N','4':'+12V'})
nic(s,'Sensor_Energy:INA228','U6','INA228AIDGSR',(272,378),'Package_SO:TSSOP-10_3x3mm_P0.5mm',{'1':'+3V3_PI','2':'I2C_SDA','3':None,'4':'I2C_SDA','5':'I2C_SCL','6':'+3V3_PI','7':'GND','8':'+12V','9':'SH12_N','10':'SH12_P'},ds='https://www.ti.com/lit/ds/symlink/ina228.pdf')
g=Grid(s,305,367,20.32,30.48,330); g.C('100nF','+3V3_PI','GND'); tp(g,'+12V')
for nm,x in (('Jetson',360),('Pi 5 V buck',400),('fans',440)):
    conn(2,'12 V to '+nm,(x,378),MF2,['+12V','GND'])
conn(5,'Pi: 3V3, SDA, SCL, GND, E-stop pressed (low)',(490,378),GH(5),['+3V3_PI','I2C_SDA','I2C_SCL','GND','ESTOP_PI_N'],ref='J21')
g=Grid(s,515,367,20.32,30.48,530); g.F('+3V3_PI')
note('''Ahead of the E-stop: motors off, compute stays up. Soft start in block 5a, BRK_PG on R.C keeps the brick off until its input is up.
-Vout is tied to GND here: the robot has one ground, P-. RS+ and RS- tied at the pins, Trim open. Input caps at the pins; the EMC filter is left out.
150 W at 12 V, 89 %: 18 W at full load, about 6 W at 50 W. Derating starts near 30 C at 20 LFM: fit a heatsink on the baseplate, in the Z 67 strip, see REV_B_PARTS. 12.7 mm tall: keep clear of the fan gap.
INA228 addresses: L arm 0x40, R arm 0x41, waist 0x42, aux 0x43, L leg 0x44, R leg 0x45, 12 V 0x46, SPARE_SW 0x47, MID 0x48. Powered by the Pi's 3.3 V; Pi pull-ups on I2C.''',(25.4,404))


# ---------- brick soft start ----------
head('5a. Brick soft start (ahead of the E-stop switch)',(25.4,440))
nic(s,'Device:R_Shunt','RS9','5mR CSS4J-4026K-5L00F',(60,462),SH,{'1':'BAT','2':'SH9_P','3':'SH9_N','4':'BRK_D'})
nic(s,Qb,'Q7','IPB017N10N5LFATMA1',(115,462),'Package_TO_SOT_SMD:TO-263-7_TabPin4',{'1':'BRK_G1','2':'BRK_IN','3':'BRK_IN','4':'BRK_D','5':'BRK_IN','6':'BRK_IN','7':'BRK_IN'})
nic(s,'Power_Management:LM5069MM-2','U7','LM5069MMX-2',(190,470),'Package_SO:MSOP-10_3x3mm_P0.5mm',{'1':'SH9_N','2':'SH9_P','3':'SH9_P','4':'GND','5':'GND','6':'BRK_T','7':'BRK_P','8':'BRK_PG','9':'BRK_IN','10':'BRK_GATE'},ds='https://www.ti.com/lit/ds/symlink/lm5069.pdf')
g=Grid(s,240,452,20.32,30.48,340)
g.R('10R','BRK_GATE','BRK_G1'); g.R('37.4k 1%','BRK_P','GND'); g.C('47nF C0G','BRK_T','GND',C603); g.C('100nF 100V','SH9_P','GND',C805); g.F('SH9_P'); g.F('BRK_IN'); tp(g,'BRK_IN')
note('''Same controller as the bank, so one part family. -2 retries on its own: a compute fault does not need a BMS cycle. UVLO on VIN, OVLO low: on whenever the pack is.
Power-limit start, no dV/dt network: 70 W limit (Rs 5 mOhm, same equation as U1), about 1.3 A at full pack. 4.4 uF plus the brick's own input caps (not published) charges in under 1 ms at 20 uF.
Timer 47 nF: 2.2 ms typ, 1.5-3.8 ms. Limit 9.7-12.3 A clears the worst brick draw, 4.7 A at 36 V. Brick input capacitance is unpublished: about 40 uF is the most the minimum timer covers, so measure one brick.
Fault check: 87 W is 1.6 A for up to 3.8 ms, against 12 A at 10 ms on the datasheet SOA. Q7 is the linear FET for that reason.
Compute is on whenever the BMS is on. Loss at 3 A: 0.05 W shunt, 0.03 W FET.''',(240,488))

# ---------- limbs ----------
head('6. Limb outputs: terminal pair, 1 mOhm shunt, INA228. Fuses inline in the pigtails.',(645,22))
LIMBS=[('L arm','GND','GND',0),('R arm','GND','+3V3_PI',0),('waist','GND','I2C_SDA',0),('aux','GND','I2C_SCL',0),('L leg','+3V3_PI','GND',1),('R leg','+3V3_PI','+3V3_PI',1)]
for k,(nm,a1,a0,d) in enumerate(LIMBS):
    y=34+k*40; K=k+1
    rs=f'RS{K+1}'; u=f'U{10+K}'; t=f'L{K}'
    nic(s,'Device:R_Shunt',rs,'1mR CSS4J-4026K-1L00F 6W',(660,y+12),SH,{'1':'VBUS','2':f'SH{t}_P','3':f'SH{t}_N','4':f'V_{t}'})
    nic(s,'Sensor_Energy:INA228',u,'INA228AIDGSR',(715,y+12),'Package_SO:TSSOP-10_3x3mm_P0.5mm',{'1':a1,'2':a0,'3':None,'4':'I2C_SDA','5':'I2C_SCL','6':'+3V3_PI','7':'GND','8':f'V_{t}','9':f'SH{t}_N','10':f'SH{t}_P'},ds='https://www.ti.com/lit/ds/symlink/ina228.pdf')
    gg=Grid(s,750,y+6,12,30,770); c=gg.C('100nF','+3V3_PI','GND')
    cref=f"C{s.refc['C']}"
    j1=term(nm+' +',(775,y+12),f'V_{t}'); j2=term(nm+' -',(805,y+12),'GND')
    if d:
        for r in (rs,u,cref,j1,j2): dnp(r)
note('1 mOhm: 60 A peak is 60 mV, ADCRANGE 0. 0.36 W at 19 A.\nLegs DNP: fit the same parts when they arrive.',(645,280))
s.refc['J']=21

# ---------- bank A ----------
head('7. E-stop bank A: second series bank, own driver, ahead of bank B',(560,300))
T48=custom('TPS48100-Q1','tps48100-q1',fp='marengo_revb:TPS48100_DGX_VSSOP-19',ds='https://www.ti.com/lit/ds/symlink/tps4810-q1.pdf',et={'12':'passive','14':'passive','15':'passive'})
nic(s,T48,'U8','TPS48100QDGXRQ1',(600,340),'marengo_revb:TPS48100_DGX_VSSOP-19',{'1':'EN_A','2':'GND','3':'EN_A','4':None,'5':'UVLO','6':'GND','7':'GND','8':'ISCP_A','9':'TMR_A','10':'GND','11':None,'12':'BST_A','13':'MID','14':'G1PD_A','15':'GATE_A','17':'SH_N','18':'SH_P','19':None,'20':'VS_A'},ds='https://www.ti.com/lit/ds/symlink/tps4810-q1.pdf')
nic(s,Qb,'Q8','IPB017N10N5LFATMA1',(680,330),'Package_TO_SOT_SMD:TO-263-7_TabPin4',{'1':'GA1','2':'MID','3':'MID','4':'SW_D','5':'MID','6':'MID','7':'MID'})
nic(s,Qb,'Q9','IPB017N10N5LFATMA1',(745,330),'Package_TO_SOT_SMD:TO-263-7_TabPin4',{'1':'GA2','2':'MID','3':'MID','4':'SW_D','5':'MID','6':'MID','7':'MID'})
g=Grid(s,650,362,17,30.48,830)
g.R('10k','RUN','EN_A'); g.R('100k','EN_A','GND'); g.R('100R','SH_P','VS_A'); g.C('100nF 100V','VS_A','GND',C805); g.C('2.2uF 25V','BST_A','MID',C805); g.R('1M','GATE_A','MID'); g.R('10R','GATE_A','GA1'); g.R('10R','GATE_A','GA2'); g.F('VS_A')
g.nl(392)
g.R('220k 1%','SW_D','MID',R1206); g.R('220k 1%','MID','GND',R1206); tp(g,'MID'); g.R('150R','G1PD_A','GATE_A',R603); g.R('40.2k 1%','ISCP_A','GND'); g.R('100k','TMR_A','GND'); g.C('680pF C0G','TMR_A','GND',C603)
nic(s,'Sensor_Energy:INA228','U17','INA228AIDGSR',(600,420),'Package_SO:TSSOP-10_3x3mm_P0.5mm',{'1':'I2C_SDA','2':'GND','3':None,'4':'I2C_SDA','5':'I2C_SCL','6':'+3V3_PI','7':'GND','8':'MID','9':'GND','10':'GND'},ds='https://www.ti.com/lit/ds/symlink/ina228.pdf')
g=Grid(s,790,392,17,30.48,830); g.C('100nF','+3V3_PI','GND')
wnote('''Own driver: a FET usually dies shorted gate to drain, which would pull a shared gate net to the pack and hold the other bank on. RUN is the only shared signal.
Both banks face the same way, drains toward the pack, so regen stays open. Bank B keeps U1, the shunt and the soft start.
Close order: U8 FLT holds U1 UVLO low until BST is up (26 ms typ, up to about 115 ms with 2.2 uF), so A closes first by logic, not by timing. A closes onto MID only. FLT also blocks B if U8 has no charge pump or has tripped.
Off edge: EN falls first, 4.6 us typ, 6.5 us max, then G1PD pulls 390 nC through 150R: about 10 us, so 6 A/us at 60 A, against a 100 us latch. D1 stays as the backstop. Check at the bench.
Short-circuit comparator on RS1: 40.2k gives 99 mV, 79-121 mV, 158-242 A, above U1's limit. TMR 100k + 680 pF: about 10 us, latch-off, cleared by an E-stop cycle. It covers reset with B shorted: A opens, and FLT holds B off.
Driver off holds gate to source with no path to GND, so MID can be biased: 220k + 220k and the 0.8-1.2 MOhm INA228 VBUS input give 0.43-0.47 x pack, 0.25 mA.
Self-test before reset, pack 36-54.6 V, BMS telemetry for pack voltage: U17 MID 15-26 V and U11 VBUS under 3 V is healthy. MID within 2 V of pack: A shorted. MID under 3 V: B shorted. VBUS high: both shorted, or the bus has not bled down yet, about 11 s on board caps, longer with drives.
U17 uses VBUS only, IN+ and IN- on GND. VBUS alone cannot see one shorted bank, which is why MID is read.''',(650,428),165)

# ---------- spare 12 V outputs ----------
head('8. Spare 12 V outputs',(560,472))
mkic('TPS259541DSG',[('1','dVdt','passive'),('2','EN/UVLO','input'),('3','IN','passive'),('4','IN','passive'),('5','OUT','passive'),('6','FLT','open_collector'),('7','ILM','passive'),('8','GND','power_in'),('9','EP','power_in')],'Package_SON:Texas_DSG0008A_WSON-8-1EP_2x2mm_P0.5mm_EP0.9x1.6mm','https://www.ti.com/lit/ds/symlink/tps2595.pdf','TPS2595 eFuse, 18 V 4 A 34 mOhm, 13.7 V clamp, auto-retry')
nic(s,LIBNAME+':TPS259541DSG','U9','TPS259541DSG',(600,500),'Package_SON:Texas_DSG0008A_WSON-8-1EP_2x2mm_P0.5mm_EP0.9x1.6mm',{'1':'SP_DV','2':'SP_EN','3':'+12V','4':'+12V','5':'SP_SW1','6':None,'7':'SP_ILM','8':'GND','9':'GND'},ds='https://www.ti.com/lit/ds/symlink/tps2595.pdf')
nic(s,'Device:R_Shunt','RS10','5mR CSS4J-4026K-5L00F',(660,500),SH,{'1':'SP_SW1','2':'SHSP_P','3':'SHSP_N','4':'SPARE_SW'})
nic(s,'Sensor_Energy:INA228','U10','INA228AIDGSR',(715,500),'Package_SO:TSSOP-10_3x3mm_P0.5mm',{'1':'+3V3_PI','2':'I2C_SCL','3':None,'4':'I2C_SDA','5':'I2C_SCL','6':'+3V3_PI','7':'GND','8':'SPARE_SW','9':'SHSP_N','10':'SHSP_P'},ds='https://www.ti.com/lit/ds/symlink/ina228.pdf')
conn(2,'SPARE_SW, off with the motors',(765,500),MF2,['SPARE_SW','GND'])
conn(2,'SPARE_ON, always on',(810,500),MF2,['SPARE_ON','GND'])
g=Grid(s,560,535,17,30.48,830)
g.R('100k','RUN','SP_EN'); g.R('68k','SP_EN','GND'); g.R('487R 1%','SP_ILM','GND'); g.C('3.3nF C0G','SP_DV','GND',C603); g.C('100nF 25V','+12V','GND'); g.C('22uF 25V','SP_SW1','GND',C1210); g.Z('SMAJ15A','SP_SW1','GND','Diode_SMD:D_SMA'); g.C('100nF','+3V3_PI','GND')
wnote('''SPARE_SW: RUN enables it, so it drops with the motor bank. The divider keeps 7.7 V of RUN under the 6 V EN limit. Limit 3.87-4.42 A (487R). dV/dt 3.3 nF: about 12 V/ms. One switch: a shorted U9 stays on. Current and volts at 0x47, 5 mOhm, ADCRANGE 1.
SPARE_ON: block 8a. Both spares sit behind RS8, so the 12 V meter counts them.''',(560,562),150)

head('8a. SPARE_ON eFuse',(440,425))
nic(s,LIBNAME+':TPS259541DSG','U18','TPS259541DSG',(480,450),'Package_SON:Texas_DSG0008A_WSON-8-1EP_2x2mm_P0.5mm_EP0.9x1.6mm',{'1':'SPON_DV','2':'SPON_EN','3':'+12V','4':'+12V','5':'SPARE_ON','6':None,'7':'SPON_ILM','8':'GND','9':'GND'},ds='https://www.ti.com/lit/ds/symlink/tps2595.pdf')
g=Grid(s,440,478,17,30.48,555)
g.R('100k','+12V','SPON_EN'); g.R('68k','SPON_EN','GND'); g.R('1.02k 1%','SPON_ILM','GND'); g.C('3.3nF C0G','SPON_DV','GND',C603); g.C('100nF 25V','+12V','GND')
note('Unswitched, about 2 A limit. Same part as U9: a short stops in 5 us and leaves compute up.',(440,503))

# ---------- E-stop pressed ----------
head('9. E-stop pressed: button NO contact to the Pi',(355.6,506))
g=Grid(s,368.3,530,17,30.48,460)
g.R('3.3k','+3V3_PI','ESTOP_NO',R1206); g.R('10k','ESTOP_NO','ESTOP_PI_N',R1206); g.C('1nF','ESTOP_PI_N','GND')
nic(s,'Diode:BAT54S','D6','BAT54S',(445,536),SOT23,{'1':'GND','2':'+3V3_PI','3':'ESTOP_PI_N'})
note('''Low means pressed. Pi GPIO input, no Pi pull-up needed. 3.3 V through 3.3k gives the contact 1 mA of wetting.
10k and the clamp pair protect the pin: a harness short to the 54.6 V bus puts 5 mA in the clamp.
Information only: the NC loop does the stopping.''',(355.6,566))
writelib()
s.write([('/'+ROOT,'1')])
print('ok',{k:v for k,v in s.refc.items() if not k.startswith('#')})
