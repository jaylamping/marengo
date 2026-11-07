"""Rev B power board, one flat sheet. Edit this, not the .kicad_sch. Run tools/regen.sh."""
import os,sys,textwrap
sys.path.insert(0,os.path.dirname(os.path.abspath(__file__)))
from kit import *
wfile(OUT+'/sym-lib-table').write('(sym_lib_table\n\t(version 7)\n\t(lib (name "marengo_revb")(type "KiCad")(uri "${KIPRJMOD}/marengo_revb.kicad_sym")(options "")(descr "rev B project symbols"))\n)\n')
wfile(OUT+'/fp-lib-table').write('(fp_lib_table\n\t(version 7)\n\t(lib (name "marengo_revb")(type "KiCad")(uri "${KIPRJMOD}/marengo_revb.pretty")(options "")(descr "rev B project footprints"))\n)\n')
REDCUBE='TerminalBlock_Wuerth:Wuerth_REDCUBE-THR_WP-THRSH_74651194_THR'
TPFP='TestPoint:TestPoint_Pad_D2.0mm'  # rev B5 cost pass: bare pads; the three GND points keep Keystone loops for scope clips
GH=lambda n:f'Connector_JST:JST_GH_SM0{n}B-GHS-TB_1x0{n}-1MP_P1.25mm_Horizontal'
MF2='Connector_Molex:Molex_Micro-Fit_3.0_43045-0212_2x01_P3.00mm_Vertical'
MF4='Connector_Molex:Molex_Micro-Fit_3.0_43045-0412_2x02_P3.00mm_Vertical'
MINI='Connector_Molex:Molex_Mini-Fit_Jr_5566-02A_2x01_P4.20mm_Vertical'
SH='Resistor_SMD:R_Shunt_Isabellenhuette_BVR4026'
SHK='Resistor_SMD:R_Shunt_Vishay_WSK2512_6332Metric_T1.19mm'
PIG='Connector_Wire:SolderWire-2.5sqmm_1x02_P7.2mm_D2.4mm_OD3.6mm'  # cable-tie holes beside each pair come from the layout (place.py)
INA='INA237AIDGSR'; INADS='https://www.ti.com/lit/ds/symlink/ina237.pdf'
CP='Capacitor_SMD:CP_Elec_10x10.5'
CAP100='47uF 80V EEH-ZC1K470P'
s=Sheet('root','marengo power board rev B','A0',comment='Rev B: a part goes in only if leaving it out would hurt. Parts list: hardware/electrical/pdb/REV_B_PARTS.md. Rev A stays as reference.')
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
conn(2,'Pack: 8 AWG pigtail pair to an XT90 (mates the pack XT90S), 1 BAT, 2 GND',(40.64,45.72),'marengo_revb:SolderWire-8AWG_1x02_P14mm_D4.5mm_OD8mm',['BAT','GND'],ref='J1')
s.refc['J']=2  # J2 (the second stud) is gone; later refs keep their numbers
g=Grid(s,68.58,38.1,20.32,30.48,150)
g.Z('5.0SMDJ58A','BAT','GND',SMC); g.C('1uF 100V X7R soft-term','BAT','GND',C1210); g.C('100nF 100V X7R soft-term','SH_P','GND',C805); g.F('BAT'); g.F('GND'); g.F('SH_P')
nic(s,'Device:R_Shunt','RS1','0.5mR CSS4J-4026R-L500F 5W',(177.8,48.26),SH,{'1':'BAT','2':'SH_P','3':'SH_N','4':'SW_D'})
Qb='marengo_revb:IPB017N10N5LF'
nic(s,Qb,'Q1','IPB017N10N5LFATMA1',(228.6,48.26),'Package_TO_SOT_SMD:TO-263-7_TabPin4',{'1':'G1','2':'VBUS','3':'VBUS','4':'MID','5':'VBUS','6':'VBUS','7':'VBUS'})
nic(s,Qb,'Q2','IPB017N10N5LFATMA1',(292.1,48.26),'Package_TO_SOT_SMD:TO-263-7_TabPin4',{'1':'G2','2':'VBUS','3':'VBUS','4':'MID','5':'VBUS','6':'VBUS','7':'VBUS'})
g=Grid(s,335.28,38.1,20.32,30.48,420)
g.R('10R','GATE','G1'); g.R('10R','GATE','G2')
nic(s,'Power_Management:LM5069MM-1','U1','LM5069MMX-1',(63.5,114.3),'Package_SO:MSOP-10_3x3mm_P0.5mm',{'1':'SH_N','2':'SH_P','3':'UVLO','4':'GND','5':'GND','6':'TIMER','7':'PWR','8':None,'9':'VBUS','10':'GATE'},ds='https://www.ti.com/lit/ds/symlink/lm5069.pdf')
g=Grid(s,114.3,96.52,20.32,30.48,420)
g.R('34k 1%','PWR','GND'); g.C('4.7nF C0G','TIMER','GND',C603); g.D('BAV19W','CDV','GATE'); g.C('100nF 100V C0G','CDV','GND',C1210); g.R('1k','GATE','CDV_B')
g.R('22k','RUN_B','UVLO'); g.R('33k','UVLO','GND','Resistor_SMD:R_0805_2012Metric')
Q(PNP,'MMBTA92LT1G',(275,111.76),'CDV_B','CDV','GND')
note('''The bank is the soft start for all the motor capacitance: BMS sees no inrush. Reset closes it, loop open drops it. No software in the path. Compute is not behind it.
Rs 0.5 mOhm: limit 97-123 A (VCL 48.5-61.5 mV), breaker 160-260 A (VCB 80-130 mV). Above peak draw and the BMS DOC (70 A or more). 1.8 W at 60 A.
Start: Cdv 100 nF, gate 10-22 uA, 100-220 V/s, 0.25-0.55 s to full. Bus ceiling 9.4 mF at 2.06 A hot DC SOA line (112 W, 45 K Tj rise). Drive caps unmeasured.
Power limit 652 W at 54.6 V, 480-810 W with tolerance: RPWR = 1.3e5 x Rs x (P - 1.18 mV x VDS / Rs), SNVS452G eq 10, VSNS 6 mV over the 5 mV floor. Timer 4.7 nF: 0.22 ms typ, 0.15-0.39 ms.
Fault check: 810 W is 14.8 A at 54.6 V for up to 0.39 ms. Reviewers read about 16 A at 0.82 ms, 70 C, one FET; TI scales as t^0.346, so about 20.7 A at 0.39 ms: 1.4 x. Read the real curve at layout.
FETs: 0.85 mOhm for the pair, 5.2 W hot at 60 A, 1.3 W at 30 A, the same again in bank A. UVLO low sinks the gate at 2 mA: about 170 us off.
The 2 mA also pulls VBUS down through U1's OUT-GATE zener, against the 54 uA of its internal 1 MOhm SENSE-OUT path, so VBUS reads near 0 V with the banks open.
Body diodes of both banks pass regen back to the pack with the banks open. OVLO tied low: opening on overvoltage would strand regen on the bus.
LM5069-1 latches on a fault. Clear: E-stop cycle, then reset.
BAT spike: the absorber in blocks 10 and 10d (8 x 47 uF) takes the lead current when a bank opens fast, 5.0SMDJ58A is the backstop. FETs and U1 are 100 V.
Polarity (rev B5): one soldered 8 AWG pigtail pair, side by side, to an XT90 that only mates the pack's XT90S one way (Joey: the pack run is the heaviest connector run in the robot). Nothing on the main path blocks a reversed pack: both banks' body diodes would conduct and D1 becomes the crowbar, so the keyed plug and the bring-up polarity check carry it.''',(300,92))

# ---------- control rail and latch ----------
head('2. Control rail and E-stop latch (no logic IC)',(25.4,172))
nic(s,LDO,'U2','TPS7A4001DGNR',(63.5,200),'Package_SO:HVSSOP-8-1EP_3x3mm_P0.65mm_EP1.57x1.89mm',{'1':'VCTL','2':'FB_CTL','3':None,'4':'GND','5':'BAT_CTL','6':None,'7':None,'8':'BAT_CTL','9':'GND'})
g=Grid(s,114.3,185.42,20.32,30.48,200)
g.C('10uF 50V X7R','VCTL','GND',C1210); g.R('464k','VCTL','FB_CTL'); g.R('49.9k','FB_CTL','GND'); tp(g,'VCTL')
note('12.1 V, 11 mA. On the battery side, so regen through the body diodes keeps it up with the BMS open. Runs the latch and the clamp. 0.5 W at 54.6 V.\nFed through F1 with its own 1 uF, clamped by a 15 V TVS, reverse diode OUT to IN: block 10.',(114.3,232))
conn(8,'E-stop: 1 NC out, 2 GND, 3 NC return, 4 GND, 5-6 reset NO, 7 GND, 8 button NO',(241.3,200),GH(8),['LOOP_OUT','GND','LOOP_RET','GND','RST_SRC','RST_IN','GND','ESTOP_NO'],ref='J20')
g=Grid(s,284.48,185.42,20.32,30.48,520)
g.R('680R 0.5W','VCTL','LOOP_OUT','Resistor_SMD:R_2010_5025Metric'); g.Z('SMAJ15A','LOOP_RET','GND','Diode_SMD:D_SMA'); g.R('4.7k','VCTL','RST_SRC',R603); g.R('10k','RST_IN','GND'); g.C('2.2nF C0G','RST_IN','RST_C',C603); g.R('1k','RST_C','L_B')
Q(PNP,'MMBT3906',(419.1,205.74),'L_PB','LOOP_RET','RUN')
Q(NPN,'MMBT3904',(469.9,205.74),'L_B','L_E','L_PC')  # emitter through the watchdog switch (block 2a)
g=Grid(s,505,185.42,20.32,30.48,630)
g.R('100k','LOOP_RET','L_PB'); g.R('10k','L_PB','L_PC'); g.R('1.2k','RUN','GND',R603); g.R('10k','RUN','L_B'); g.R('10k','L_B','GND')
g.nl(); g.C('10nF 50V X7R','L_B','GND'); tp(g,'RUN')
note('''Seal-in latch, ngspice checked at hFE 80: stays off at power-up even with reset held, reset closes it, a 50 us contact bounce rides through,
loop open drops it in about 100 us, re-closing the loop does nothing, a reset held through a loop cycle does nothing, a new press works 5 ms after release.
RUN 7 V. UVLO and EN_A each take RUN through their own diode (block 10), so a fault on either pin cannot drive RUN. UVLO 3.8 V; needs about 4.8 V on RUN_B rising. Loses VCTL, RUN drops, UVLO drops: bank opens.
Loop 6 mA, wetting for a control-rated NC contact. No 12 V on the connector without a series R. GND between loop out and return, and between return and reset: a short there opens the loop or does nothing.
Brake resistor NC thermostat: wire it in series in the loop harness, no board parts.
Button HB2-BS545, 22 mm mushroom, twist release, 1NC + 1NO, 660 V AC 10 A, control-rated only: NC in this loop, NO to the Pi (block 9). RUN also enables bank A (block 7) and SPARE_SW (block 8).''',(284.48,240))

# ---------- clamp ----------
head('3. Braking clamp: 56.6-56.8 V on, 55.8-55.9 V off (typ), 57.5 V worst case, into 2 x HS50 20R in parallel',(25.4,262))
nic(s,TLV,'U3','TLV1805DBVR',(63.5,292),'Package_TO_SOT_SMD:SOT-23-6',{'1':'CL_OUT','2':'GND','3':'VREF_CL','4':'CL_SENSE','5':'GND','6':'VCTL'})
nic(s,'Reference_Voltage:LM4040DBZ-2.5','U4','LM4040AIM3-2.5',(127,292),SOT23,{'1':'VREF_CL','2':'GND','3':None})
g=Grid(s,152.4,277,20.32,30.48,290)
g.R('9.09k','VCTL','VREF_CL'); g.C('100nF 50V X7R','VCTL','GND',C603); g.R('499k 0.1%','VBUS','CL_DIV',R1206); g.R('604k 0.1%','CL_DIV','CL_SENSE',R1206); g.R('51.1k 0.1%','CL_SENSE','GND'); g.R('9.53M','CL_OUT','CL_HY',R603); g.C('100pF C0G','CL_SENSE','GND')
g.nl(); g.R('10R','CL_OUT','CL_G'); g.R('100k','CL_G','GND'); g.D('MBRS3100T3G','VBUS','CL_D',SMC)
nfet5(s,'Q6','BSC040N10NS5ATMA1',(254,322),'CL_D','CL_G','GND')
conn(2,'HS50 A (+X): 1 VBUS, 2 FET drain (Mini-Fit Jr, 18 AWG)',(330.2,292),MINI,['VBUS','CL_D'],ref='J3'); s.refc['J']=4  # J4 is gone
note('''Two Arcol HS50 20R J in parallel, 10 Ohm total, one on each gusset (+X: J3/J4; -X: J25/J26, block 10c; place all four on the board's top edge, +X half, because the -X leads cross under the board, about 150 mm longer), on the 5 mm countersunk Al plates with thermal compound. Each lead pair is twisted. At the 56.8 V threshold: 5.7 A, 322 W total, 2.84 A and 161 W per resistor. MBRS3100 carries the lead and winding inductance at turn-off, so lead length sets EMI, not FET stress. Each is rated 50 W only on Arcol's 535 cm2 plate, so it works on burst energy: 32 g each, about 29 J/K, so a 3 kJ burst is about +50 C per body. Software caps braking at 50 W average over 60 s and 3 kJ per burst, total. Thermostats: one 67L120 under each body, both in series in the E-stop loop (rev B5), so an open resistor can't leave the other unwatched. Threshold values are rev A's, unchanged.
Self-powered: VCTL lives as long as the bus or the pack does. If the FET sticks on, the resistor's thermostat opens the loop and the bank drops the bus.
TLV1805 drives the FET directly: 100 mA peak, 0.3 us into 4 nF. Rev A's driver, meter, shunt, status FET and NTC are gone, and a smaller FET replaces the IPT015.
Leg stage later: fit 2 Ohm and recheck this FET, 29 A gives about 5 W.''',(355.6,280))

# ---------- bus caps ----------
head('4. Bus capacitors',(355.6,308))
g=Grid(s,368.3,330,38.1,30.48,560)
for _ in range(4): g.CP(CAP100,'VBUS','GND',CP)
g.F('VBUS'); tp(g,'VBUS')
note('235 uF, 80 V: 5 x Panasonic EEH-ZC1K470P hybrid, 36 mOhm, 10 x 10.2 mm, active line (rev B5: the APZ 68 uF is obsolete); the fifth is in block 10d. The drives have their own bulk. VBUS peaks about 58 V: 27 % margin. Regen overshoot about 0.8 V before the clamp acts.',(355.6,318))

# ---------- brick ----------
head('5. 12 V converter: own LM5146 buck board (marengo_buck12) on the -X rail, pigtail J24 (block 10); DVE0125V2NBC is the drop-in fallback',(25.4,352))
g=Grid(s,114.3,367,20.32,30.48,200)
g.C('2.2uF 100V','BRK_IN','GND',C1210,k=2); g.C('22uF 25V','+12V_RAW','GND',C1210,k=2)
nic(s,'Device:R_Shunt','RS8','2mR 2512 Kelvin WSK25122L000FEA',(218,378),SHK,{'1':'+12V_RAW','2':'SH12_P','3':'SH12_N','4':'+12V'})
nic(s,'Sensor_Energy:INA228','U6',INA,(272,378),'Package_SO:TSSOP-10_3x3mm_P0.5mm',{'1':'I2C_SCL','2':'+3V3_PI','3':None,'4':'I2C_SDA','5':'I2C_SCL','6':'+3V3_PI','7':'GND','8':'+12V','9':'SH12_N','10':'SH12_P'},ds=INADS)
g=Grid(s,305,367,20.32,30.48,330); g.C('100nF','+3V3_PI','GND'); tp(g,'+12V')
for nm,x,n in (('Jetson',360,'+12V_JET'),('Pi 5 V buck',400,'+12V_PI'),('fans',440,'+12V_FAN')):
    conn(2,'12 V to '+nm+' (fused, block 10c)',(x,378),MF2,[n,'GND'])
conn(7,'Pi: 3V3, SDA, GND, SCL, E-stop pressed (low), heartbeat out, port fault (low)',(490,378),GH(7),['+3V3_PI','I2C_SDA','GND','I2C_SCL','ESTOP_PI_N','WD_FEED','PORT_FLT_N'],ref='J21')
g=Grid(s,515,367,20.32,30.48,530); g.F('+3V3_PI')
note('''Ahead of the E-stop: motors off, compute stays up. Soft start in block 5a; the converter's negative-logic ON/OFF is held off until U7's PGD is good (block 10).
Off board on the top front rail (frame 3877317), baseplate down, 4x M3 into its threaded baseplate (50.8 x 15.24 mm, 3 mm max depth). J24 sits on the board's top edge, -X half. -Vin and -Vout both go to GND: the robot has one ground, P-.
Pigtail about 150 mm, twisted pairs, 18 AWG: 2 wires each way on the input, 3 each way on +12 V (0.1 V drop at 12 A including contacts). At the converter end: 47 uF 100 V aluminium (its ESR damps the lead, 75-150 nH) plus 1 uF across +VIN/-VIN at the pins. 300 W, 25 A, 94.6 %: about 9 W at the 140 W ceiling, into the plate.
Input UVLO 32-36 V on, 30-34 V off, fixed on the DVE: near an empty pack under sag, compute can drop before the BMS cuts out.
INA228 addresses, no strap on SDA: L arm 0x40, R arm 0x41, aux 0x43, L leg 0x44, R leg 0x45, SPARE_SW 0x47, waist 0x4C, 12 V 0x4D, MID 0x4F. Powered by the Pi's 3.3 V; Pi pull-ups on I2C.''',(25.4,424))


# ---------- brick soft start ----------
head('5a. Brick soft start (ahead of the E-stop switch)',(25.4,440))
nic(s,'Device:R_Shunt','RS9','5mR 2512 Kelvin WSK2512R0050FEA',(60,462),SHK,{'1':'BAT','2':'SH9_P','3':'SH9_N','4':'BRK_D'})
nic(s,Qb,'Q7','IPB017N10N5LFATMA1',(115,462),'Package_TO_SOT_SMD:TO-263-7_TabPin4',{'1':'BRK_G1','2':'BRK_IN','3':'BRK_IN','4':'BRK_D','5':'BRK_IN','6':'BRK_IN','7':'BRK_IN'})
nic(s,'Power_Management:LM5069MM-2','U7','LM5069MMX-2',(190,470),'Package_SO:MSOP-10_3x3mm_P0.5mm',{'1':'SH9_N','2':'SH9_P','3':'SH9_P','4':'GND','5':'GND','6':'BRK_T','7':'BRK_P','8':'BRK_PG','9':'BRK_IN','10':'BRK_GATE'},ds='https://www.ti.com/lit/ds/symlink/lm5069.pdf')
g=Grid(s,240,452,20.32,30.48,340)
g.R('10R','BRK_GATE','BRK_G1'); g.R('37.4k 1%','BRK_P','GND'); g.C('220nF C0G','BRK_T','GND','Capacitor_SMD:C_1206_3216Metric'); g.C('100nF 100V X7R soft-term','SH9_P','GND',C805); g.F('SH9_P'); g.F('BRK_IN'); tp(g,'BRK_IN')
wnote('''Same controller as the bank, so one part family. -2 retries on its own: a compute fault does not need a BMS cycle. UVLO on VIN, OVLO low: on whenever the pack is.
Power-limit start, no dV/dt network: 70 W limit (Rs 5 mOhm, same equation as U1), about 1.3 A at full pack. 4.4 uF plus the brick's own input caps (not published) charges in under 1 ms at 20 uF.
Timer 100 nF: 4.7 ms typ, 3.1-8 ms. Limit 9.7-12.3 A clears the converter's worst draw, about 4.2 A at 36 V. It charges about 48 uF at the converter end plus the DVE's unpublished internal capacitance at the low power limit.
Fault check: 87 W is 1.6 A for up to 3.8 ms, against 12 A at 10 ms on the datasheet SOA. Q7 is the linear FET for that reason.
Compute is on whenever the BMS is on. Loss at 3 A: 0.05 W shunt, 0.03 W FET.''',(240,486),100)

# ---------- limbs ----------
head('6. Limb ports: 1.5 mOhm shunt, own breaker (block 6a), meter, soldered pigtail pair. Pigtail fuses inline.',(645,22))
LIMBS=[('L arm','GND','GND',0),('R arm','GND','+3V3_PI',0),('waist','I2C_SCL','GND',0),('aux','GND','I2C_SCL',1),('L leg','+3V3_PI','GND',1),('R leg','+3V3_PI','+3V3_PI',1)]
PORTDNP=[]
for k,(nm,a1,a0,d) in enumerate(LIMBS):
    y=34+k*40; K=k+1
    rs=f'RS{K+1}'; u=f'U{10+K}'; t=f'L{K}'
    nic(s,'Device:R_Shunt',rs,'1.5mR 2512 Kelvin WSK25121L500FEA',(660,y+12),SHK,{'1':'VBUS','2':f'SH{t}_P','3':f'SH{t}_N','4':f'D_{t}'})
    vb=f'D_{t}' if t=='L2' else f'V_{t}'   # R arm meter reads the port input: VBUS even with every port off
    nic(s,'Sensor_Energy:INA228',u,INA,(715,y+12),'Package_SO:TSSOP-10_3x3mm_P0.5mm',{'1':a1,'2':a0,'3':None,'4':'I2C_SDA','5':'I2C_SCL','6':'+3V3_PI','7':'GND','8':vb,'9':f'SH{t}_N','10':f'SH{t}_P'},ds=INADS)
    gg=Grid(s,750,y+6,12,30,770); c=gg.C('100nF 25V X7R','+3V3_PI','GND')
    cref=f"C{s.refc['C']}"
    j1=conn(2,nm+': 14 AWG pigtail pair, 1 +, 2 return',(775,y+12),PIG,[f'V_{t}','GND'])
    if d:
        PORTDNP.append(K)
        for r in (rs,u,cref,j1): dnp(r)
note('1.5 mOhm: 45 A is 68 mV, ADCRANGE 0. 0.6 W at 20 A.\nEach limb\'s + and return leave on one soldered pair, twisted from the board edge. No GND studs.\nAux and legs DNP: fit the same parts when they arrive (legs on rev C).\nU12 reads the R arm port input, so it shows VBUS with every port off; the other meters read their port outputs.',(645,280))
s.refc['J']=21

# ---------- bank A ----------
head('7. E-stop bank A: second series bank, own driver, ahead of bank B',(560,300))
T48=custom('TPS48100-Q1','tps48100-q1',fp='marengo_revb:TPS48100_DGX_VSSOP-19',ds='https://www.ti.com/lit/ds/symlink/tps4810-q1.pdf',et={'12':'passive','14':'passive','15':'passive'})
nic(s,T48,'U8','TPS48100QDGXRQ1',(600,340),'marengo_revb:TPS48100_DGX_VSSOP-19',{'1':'EN_A','2':'GND','3':'EN_A','4':None,'5':'FLT_A','6':'GND','7':'GND','8':'ISCP_A','9':'TMR_A','10':'GND','11':None,'12':'BST_A','13':'MID','14':'G1PD_A','15':'GATE_A','17':'CSN_A','18':'CSP_A','19':None,'20':'VS_A'},ds='https://www.ti.com/lit/ds/symlink/tps4810-q1.pdf')
nic(s,Qb,'Q8','IPB017N10N5LFATMA1',(680,330),'Package_TO_SOT_SMD:TO-263-7_TabPin4',{'1':'GA1','2':'MID','3':'MID','4':'SW_D','5':'MID','6':'MID','7':'MID'})
nic(s,Qb,'Q9','IPB017N10N5LFATMA1',(745,330),'Package_TO_SOT_SMD:TO-263-7_TabPin4',{'1':'GA2','2':'MID','3':'MID','4':'SW_D','5':'MID','6':'MID','7':'MID'})
g=Grid(s,650,362,17,30.48,830)
g.R('10k','RUN_A','EN_A'); g.R('100k','EN_A','GND'); g.R('100R','SH_P','VS_A'); g.C('100nF 100V','VS_A','GND',C805); g.C('2.2uF 25V','BST_A','MID',C805); g.R('1M','GATE_A','MID'); g.R('10R','GATE_A','GA1'); g.R('10R','GATE_A','GA2'); g.F('VS_A')
g.nl(392)
g.R('47k 1%','SW_D','MID',R1206); g.R('47k 1%','MID','GND',R1206); tp(g,'MID'); g.R('22R','G1PD_A','GATE_A',R603); g.R('40.2k 1%','ISCP_A','GND'); g.R('100k','TMR_A','GND'); g.C('680pF C0G','TMR_A','GND',C603)
nic(s,'Sensor_Energy:INA228','U17',INA,(600,420),'Package_SO:TSSOP-10_3x3mm_P0.5mm',{'1':'I2C_SCL','2':'I2C_SCL','3':None,'4':'I2C_SDA','5':'I2C_SCL','6':'+3V3_PI','7':'GND','8':'MID_S','9':'GND','10':'CL_TM'},ds=INADS)
g=Grid(s,790,392,17,30.48,830); g.C('100nF','+3V3_PI','GND')
wnote('''Own driver: a FET usually dies shorted gate to drain, which would pull a shared gate net to the pack and hold the other bank on. RUN is the only shared signal, and each bank takes it through its own diode.
Both banks face the same way, drains toward the pack, so regen stays open. Bank B keeps U1, the shunt and the soft start.
Close order: U8 FLT holds U1 UVLO low (about 1.9 V, through BAT46W + 10k) until BST is up (about 57 ms typ, 35-115 ms with 2.2 uF), so A closes first by logic, not by timing. The diode only lets FLT pull down: a failed-high FLT can't enable B. A closes onto MID only. FLT also blocks B if U8 has no charge pump or has tripped.
Off edge: EN falls first, 4.6 us typ, 6.5 us max, then G1PD pulls 390 nC through 22R: about 2-3 us. On a bus short U1's breaker can open B first, so the spike can land on MID with A on: the absorber (block 10) bounds BAT, and so MID, and U17 has its own clamp.
Short-circuit comparator on RS1: 40.2k gives 99 mV, 158-242 A, above U1's limit. CS+/CS- through 0R with a DNP cap: TI's RC placeholder against nuisance trips. TMR 100k only (C29 DNP): fastest response, 4.3 us, still latch-off, cleared by an E-stop cycle. It covers reset with B shorted: A opens, and FLT holds B off.
Driver off holds gate to source with no path to GND, so MID can be biased: 47k + 47k, 0.47 x pack nominal, 0.6 mA, 16 mW each. Hot FET leakage (200 uA per bank at 125 C) moves MID by up to about 4 V. U17 reads MID through 33k with a 68 V zener at the pin (85 V max): calibrate the 3 % gain.
Self-test before reset, pack 36-54.6 V, BMS telemetry for pack voltage: MID 0.33-0.60 x pack and VBUS under 3 V is healthy (U11 reads V_L1: use the highest limb meter). MID within 2 V of pack: A shorted. MID under 3 V: B shorted. VBUS high: both shorted, or the bus has not bled down yet, about 11 s on board caps, longer with drives.
U17 reads MID on its VBUS pin, and clamp duty on IN+ (block 10d). VBUS alone cannot see one shorted bank, which is why MID is read.''',(650,428),165)

# ---------- spare 12 V outputs ----------
head('8. Spare 12 V outputs',(560,472))
mkic('TPS259541DSG',[('1','dVdt','passive'),('2','EN/UVLO','input'),('3','IN','passive'),('4','IN','passive'),('5','OUT','passive'),('6','FLT','open_collector'),('7','ILM','passive'),('8','GND','power_in'),('9','EP','power_in')],'Package_SON:Texas_DSG0008A_WSON-8-1EP_2x2mm_P0.5mm_EP0.9x1.6mm','https://www.ti.com/lit/ds/symlink/tps2595.pdf','TPS2595 eFuse, 18 V 4 A 34 mOhm, 13.7 V clamp, auto-retry')
nic(s,LIBNAME+':TPS259541DSG','U9','TPS259541DSGR',(600,500),'Package_SON:Texas_DSG0008A_WSON-8-1EP_2x2mm_P0.5mm_EP0.9x1.6mm',{'1':'SP_DV','2':'SP_EN','3':'+12V','4':'+12V','5':'SP_SW1','6':None,'7':'SP_ILM','8':'GND','9':'GND'},ds='https://www.ti.com/lit/ds/symlink/tps2595.pdf')
nic(s,'Device:R_Shunt','RS10','5mR 2512 Kelvin WSK2512R0050FEA',(660,500),SHK,{'1':'SP_SW1','2':'SHSP_P','3':'SHSP_N','4':'SPARE_SW'})
nic(s,'Sensor_Energy:INA228','U10',INA,(715,500),'Package_SO:TSSOP-10_3x3mm_P0.5mm',{'1':'+3V3_PI','2':'I2C_SCL','3':None,'4':'I2C_SDA','5':'I2C_SCL','6':'+3V3_PI','7':'GND','8':'SPARE_SW','9':'SHSP_N','10':'SHSP_P'},ds=INADS)
conn(4,'SPARE_SW, off with the motors (2 x 2, keyed apart from the 2-pin outputs)',(765,500),MF4,['SPARE_SW','GND','SPARE_SW','GND'])
conn(2,'SPARE_ON, always on',(810,500),MF2,['SPARE_ON','GND'])
g=Grid(s,560,535,17,30.48,830)
g.R('100k','RUN','SP_EN'); g.R('68k','SP_EN','GND'); g.R('499R 1%','SP_ILM','GND'); g.C('3.3nF C0G','SP_DV','GND',C603); g.C('100nF 25V','+12V','GND'); g.C('22uF 25V','SP_SW1','GND',C1210); g.D('B340A-13-F','SP_SW1','GND','Diode_SMD:D_SMA'); g.C('100nF','+3V3_PI','GND')
wnote('''SPARE_SW: RUN enables it, so it drops with the motor bank. The divider keeps 7.7 V of RUN under the 6 V EN limit. Limit 3.87-4.42 A (487R). dV/dt 3.3 nF: about 12 V/ms. One switch: a shorted U9 stays on. Current and volts at 0x47, 5 mOhm, ADCRANGE 1.
SPARE_ON: block 8a. Both spares sit behind RS8, so the 12 V meter counts them.''',(560,562),150)

head('8a. SPARE_ON eFuse',(440,425))
nic(s,LIBNAME+':TPS259541DSG','U18','TPS259541DSGR',(480,450),'Package_SON:Texas_DSG0008A_WSON-8-1EP_2x2mm_P0.5mm_EP0.9x1.6mm',{'1':'SPON_DV','2':'SPON_EN','3':'+12V','4':'+12V','5':'SPARE_ON','6':None,'7':'SPON_ILM','8':'GND','9':'GND'},ds='https://www.ti.com/lit/ds/symlink/tps2595.pdf')
g=Grid(s,440,478,17,30.48,555)
g.R('100k','+12V','SPON_EN'); g.R('33k','SPON_EN','GND'); g.R('1.02k 1%','SPON_ILM','GND'); g.C('3.3nF C0G','SPON_DV','GND',C603); g.C('100nF 25V','+12V','GND')
note('Unswitched, about 2 A limit. Same part as U9: a short stops in 5 us and leaves compute up.',(440,503))

# ---------- E-stop pressed ----------
head('9. E-stop pressed: button NO contact to the Pi',(355.6,506))
g=Grid(s,368.3,530,17,30.48,460)
g.R('3.3k','NO_PU','ESTOP_NO',R1206); g.R('100k','ESTOP_NO','ESTOP_PI_N',R1206); g.C('1nF 50V X7R','ESTOP_PI_N','GND')
nic(s,'Diode:BAT54S','D6','BAT54S',(445,536),SOT23,{'1':'GND','2':'+3V3_PI','3':'ESTOP_PI_N'})
note('''Low means pressed. Pi GPIO input, no Pi pull-up needed. 3.3 V through 3.3k gives the contact 1 mA of wetting.
A harness short to the 54.6 V bus: the BAT46W blocks the pull-up (block 10), and 100k leaves 0.5 mA in the clamp pair. Open contact reads about 3.0 V.
Information only: the NC loop does the stopping.''',(355.6,566))

# ---------- rev B2 fixes ----------
head('10. Rev B2 protection parts (refs appended so earlier refs keep their numbers)',(25.4,515))
s.refc['D']=max(s.refc.get('D',0),6)
g=Grid(s,38.1,530,25.4,30.48,340)
g.D('SK810L-TP','ABS','BAT',SMC); g.CP('47uF 80V EEH-ZC1K470P','ABS','ABS_N','Capacitor_SMD:CP_Elec_10x10.5'); g.R('100k','ABS','GND',R1206)
g.two('Device:Fuse','F','250mA 125V 0466.250','BAT','BAT_F','Fuse:Fuse_1206_3216Metric'); g.C('1uF 100V','BAT_CTL','GND',C1210); g.F('BAT_CTL'); g.D('ES1D','BAT_CTL','BAT_R','Diode_SMD:D_SMA'); g.Z('SMAJ15A','VCTL','GND','Diode_SMD:D_SMA')
g.D('BAV19W','RUN_B','RUN'); g.D('BAV19W','RUN_A','RUN'); g.Z('BZT52C12','FLT_A','GND')
g.nl(); g.R('33k','MID','MID_S'); g.C('22nF 100V','MID','GND',C805); g.D('BAT46W','NO_PU','+3V3_PI'); g.CP('330uF 25V EEH-ZA1E331P','+12V','GND','Capacitor_SMD:CP_Elec_10x10.5')
g.C('1uF 50V X7R','RST_SRC','GND',C805); g.Z('BZT52C10','MID','MID_Z'); g.R('220k','MID_Z','MID_B','Resistor_SMD:R_0805_2012Metric'); g.R('220k','MID_B','GND'); g.R('100k','VCTL','INT_B')
s.refc['Q']=max(s.refc.get('Q',0),9)
Q(NPN,'MMBT3904',(268,562),'INT_B','GND','RST_SRC'); Q(NPN,'MMBT3904',(295,562),'MID_B','GND','INT_B')
# 12 V converter pigtail: the DVE moved off the board (Joey, 2026-11-07)
conn(12,'12 V converter pigtail: 1-2 +VIN, 3-4 -VIN, 5 ON/OFF (low = on), 6 -VIN, 7-9 -VOUT, 10-12 +VOUT',(66,385),'Connector_Molex:Molex_Micro-Fit_3.0_43045-1212_2x06_P3.00mm_Vertical',['BRK_F','BRK_F','GND','GND','BRK_ON','GND','GND','GND','GND','+12V_RAW','+12V_RAW','+12V_RAW'])
g=Grid(s,114.3,398,20.32,30.48,200)
g.two('Device:Fuse','F','20A 58V MINI 0997020.WXN','BRK_IN','BRK_F','Fuse:Fuseholder_Blade_Mini_Keystone_3568'); g.R('470k','BRK_IN','BRK_PG',R1206); g.Z('BZT52C12','BRK_PG','GND'); g.F('+12V_RAW')
nic(s,'Transistor_FET:Q_NMOS_GSD',s.nref('Q'),'2N7002',(240,402),SOT23,{'1':'BRK_PG','2':'GND','3':'BRK_OND'})
g.R('10k 0.75W','FLT_D','UVLO','Resistor_SMD:R_2010_5025Metric')
head('10b. Absorber close option (M3): 0R fitted, the rest DNP',(355.6,445))
g=Grid(s,362,458,17,30.48,445)
g.R('copper link','ABS_N','GND','NetTie:NetTie-2_SMD_Pad2.0mm')  # R53: direct copper (Joey, 2026-11-07), at least 5 mm wide in layout; cut it only to fit 10b
g.R('2R 2512 pulse-rated 100mJ','ABS_N','GND','Resistor_SMD:R_2512_6332Metric'); dnp(f"R{s.refc['R']}")
g.R('100k','VCTL','PRE_G'); dnp(f"R{s.refc['R']}")
g.C('100nF','PRE_G','GND'); dnp(f"C{s.refc['C']}")
g.R('1M','PRE_G','GND'); dnp(f"R{s.refc['R']}")
q=s.nref('Q'); nfet5(s,q,'BSC040N10NS5ATMA1',(400,492),'ABS_N','PRE_G','GND'); dnp(q)
wnote('''Block 10. Absorber: a bank trip into a bus short cuts 350-890 A. tools/absorber_sim.py (pack 54.6 V, no pack yet), worst corners: 0.05 Ohm pulse loop (cell AC resistance), 0.5-1 uH, D1 at 71.2 V, C -20 %, trip 8.8-17.5 us. Eight 47 uF 80 V hybrids (EEH-ZC1K470P, the bus-cap part) give BAT 75 V or less (0.05 Ohm loop): 6 % under rating, about 18 % under surge (rev B5: the APZ 100 V line is obsolete). Bring-up step 1 is a gate: a pulse loop under 0.05 Ohm means rerun absorber_sim before motors. Measure lead L and pulse R at bring-up. 100k bleeds it. BMS close drives about 300 A into three cans for a few us: bench it on the JK.
U2 feed: F1, 22R, ES1D in series, then 1 uF. A shorted U2 puts the pack on VCTL: 22R holds it to about 1.6 A, so SMAJ15A clamps under 19.2 V (Q6 gate 20 V) and F1 (50 A breaking) opens. The series diode keeps C5 from back-driving U2 when BAT collapses, and 22R keeps C39's inrush far under F1's I2t.
Absorber close option (M3), DNP: if the BMS trips when it closes onto the absorber, remove the 0R and fit 2R + the delayed NMOS + its RC: 25 A at close, clamp unchanged (absorber_sim.py).
RUN_A and RUN_B diodes: a U1 pin 2-3 bridge or an EN fault can't drive RUN. FLT reaches UVLO through BAT46W + 10k (2010) with a 12 V zener on FLT_A: FLT low holds UVLO near 1.9 V; a bridged UVLO puts at most 5.5 mA into FLT and 12.7 V on it (15 V recommended).
Converter: F2 20 A on the board end protects the pigtail (U7 already limits at 9.7-12.3 A; F2 backs up a shorted Q7). The DVE has no reverse protection, so the keyed connector carries polarity. U7 PGD high (good) turns on the 2N7002, which pulls ON/OFF to -Vin through 220R: on (0.45 V at 2 mA). 220R limits a pin 5-11 short to 55 mA. 470k and the 12 V zener bias the gate from BRK_IN, so it doesn't depend on VCTL.
1 uF on RST_SRC (M8): a reset held at power-up no longer arms the latch (tools/latch_sim.py, 36 cases). Reset interlock (M1): Q10 shorts RST_SRC until MID is over about 11 V (10 V zener, Q11), and Q14 shorts it while VBUS is over 6-8.5 V (5.1 V zener, hFE spread). Operator rule: wait for VBUS under 3 V. Reset needs MID healthy and the bus down, so a shorted B can't be hard-closed onto a charged bus. After a stop, wait for the drives' caps to bleed. It loads MID by about 30 uA. 33k on U17 VBUS (M13). 22 nF SRC to GND, TI's recommendation for U8. BAT46W (M4). 330 uF on +12V: spare-output shorts and servo back-feed (M9) land in it and the compute loads.''',(300,128),175)
# M1 (Joey, 2026-11-07): reset also needs the bus down. Q14 shorts RST_SRC while VBUS is over about 6 V. Last, so earlier refs keep their numbers.
g=Grid(s,322,530,17,30.48,352); g.Z('BZT52C5V1','VBUS','VB_Z'); g.R('220k','VB_B','GND',R603); g.nl(); g.R('47k','VB_Z','VB_B','Resistor_SMD:R_0805_2012Metric')
Q(NPN,'MMBT3904',(345,562),'VB_B','GND','RST_SRC')
# Rev B4 review fixes, appended so earlier refs keep their numbers
head('10c. Rev B4 review parts',(25.4,610))
g=Grid(s,38.1,625,40.64,35.56,560)
g.CP('47uF 80V EEH-ZC1K470P','ABS','ABS_N','Capacitor_SMD:CP_Elec_10x10.5'); g.CP('47uF 80V EEH-ZC1K470P','ABS','ABS_N','Capacitor_SMD:CP_Elec_10x10.5')
g.R('22R CRCW251222R0FKEGHP','BAT_F','BAT_R','Resistor_SMD:R_2512_6332Metric'); g.D('BAT46W','FLT_A','FLT_D'); g.Z('BZT52H-C68,115','MID_S','GND','Diode_SMD:D_SOD-123F'); g.R('220R','BRK_OND','BRK_ON',R603)
g.R('0R','SH_P','CSP_A',R603); g.R('0R','SH_N','CSN_A',R603); g.C('1nF C0G','CSP_A','CSN_A',C603); dnp(f"C{s.refc['C']}")
g.D('B340A-13-F','SPARE_ON','GND','Diode_SMD:D_SMA'); g.R('4.7k 2W','VBUS','GND','Resistor_SMD:R_2512_6332Metric'); dnp(f"R{s.refc['R']}")
g.two('Device:Fuse','F','5A 32V 0466005','+12V','+12V_JET','Fuse:Fuse_1206_3216Metric'); g.two('Device:Fuse','F','5A 32V 0466005','+12V','+12V_PI','Fuse:Fuse_1206_3216Metric'); g.two('Device:Fuse','F','2A 32V 0466002','+12V','+12V_FAN','Fuse:Fuse_1206_3216Metric')
g.Z('SMBJ15A','+12V','GND','Diode_SMD:D_SMB')  # servo back-feed clamp (Joey, 2026-11-07)
wnote('''Rev B4 review: two more absorber cans (three in all). U2 feed 22R and series ES1D. FLT diode BAT46W (pull-down only). 68 V zener on U17 VBUS. 220R in Q12's drain. U8 sense RC placeholder: 0R fitted, 1 nF DNP. Schottky on SPARE_ON (and D5 on SPARE_SW). VBUS bleeder footprint, DNP: fit it only if the drives don't bleed the bus. Fuses on the Jetson, Pi buck and fan outputs (the DVE limits at about 30 A). SMBJ15A on +12V: servo back-feed clamp, 16.7 V min breakdown, so it never conducts at 12 V.''',(25.4,700),160)
conn(2,'HS50 B (-X): 1 VBUS, 2 FET drain (Mini-Fit Jr, 18 AWG)',(200,660),MINI,['VBUS','CL_D'],ref='J25')
g.R('9.53M','CL_HY','CL_SENSE',R603)  # R22 split: 19.1 MOhm isn't stocked in 0402 (rev B4 audit)
dnp('C29')  # bank A comparator at its fastest: TMR with R37 only (rev B3 review B2)

# ---------- rev B5: review round 5 parts that aren't a block of their own ----------
head('10d. Rev B5 parts',(25.4,760))
g=Grid(s,38.1,775,25.4,30.48,420)
g.CP('47uF 80V EEH-ZC1K470P','VBUS','GND','Capacitor_SMD:CP_Elec_10x10.5')   # fifth bus can (the other four are in block 4)
g.D('SK810L-TP','ABS','BAT',SMC)   # second absorber diode in parallel with D7: halves its series R (absorber_sim D7 sweep)
g.R('100k','CL_OUT','CL_TM'); g.R('1k','CL_TM','GND'); g.C('4.7uF 10V X7R','CL_TM','GND',C805)   # clamp duty into U17's shunt input
g.C('1uF 100V X7R','VBUS','GND',C1210)   # local HF cap at Q6/D4 (r5 layout 14)
g.nl()
for _ in range(3): nic(s,'Connector:TestPoint',s.nref('TP'),'TP GND',g.at(),'TestPoint:TestPoint_Keystone_5015_Micro_Mini',{'1':'GND'})
for net in ('GATE','GATE_A','UVLO','FLT_A'): nic(s,'Connector:TestPoint',s.nref('TP'),'TP '+net,g.at(),'TestPoint:TestPoint_Pad_D1.0mm',{'1':net})
# rev B5 hand assembly: probe pads on internal power nodes and the watchdog output (TP14-TP16). BAT, CL_D, the limb
# outputs, PORT_FLT_N and +3V3_PI are already exposed on J1, J3/J25, the pigtail pads and J21.
for net in ('SW_D','ABS','WDO_N'): nic(s,'Connector:TestPoint',s.nref('TP'),'TP '+net,g.at(),'TestPoint:TestPoint_Pad_D1.0mm',{'1':net})
wnote('''Fifth 47 uF 80 V hybrid on VBUS (EEH-ZC1K470P, active line; the APZ 68 uF is obsolete): 235 uF in all, the value tools/port_sim.py uses.
Second SK810L beside D7: the unread forward curve at 300-900 A was worth up to 8 V on BAT at 30 mOhm (absorber_sim D7 sweep, 94 V); two in parallel halve it, so the worst corner stays at about 91 V, as with the assumed 10 mOhm.
Clamp duty: CL_OUT (0/12.1 V) through 100k/1k into U17 IN+ (IN- on GND): 120 mV at 100 % duty, 4.7 ms RC. The Pi reads duty x 120 mV and integrates duty x VBUS^2 / 10 Ohm for the brake-energy cap.
1 uF 100 V at Q6/D4. GND test points, one per quadrant, and pads on GATE, GATE_A, UVLO, FLT_A for the close-order scope test.''',(25.4,840),170)

# ---------- rev B5: limb port breakers (Opus round 5, Joey 2026-11-07) ----------
head('6a. Limb port breakers: TPS48111-Q1 + one FET per port, latch-off, enabled by RUN',(850,22))
mkic('TPS48111-Q1',[('1','EN/UVLO','input'),('2','INP_G','input'),('3','INP','input'),('4','FLT_T','open_collector'),('5','FLT_I','open_collector'),('6','GND','power_in'),('7','IMON','passive'),('8','IWRN','passive'),('9','TMR','passive'),('10','DIODE','passive'),('11','G','output'),('12','BST','passive'),('13','SRC','passive'),('14','PD','output'),('15','PU','output'),('17','CS-','input'),('18','CS+','input'),('19','ISCP','passive'),('20','VS','passive')],'marengo_revb:TPS48100_DGX_VSSOP-19','https://www.ti.com/lit/ds/symlink/tps4811-q1.pdf','100 V smart high-side driver, 1.2 us short-circuit trip, 4 A pull-down (DGX-19, same land as TPS48100)')
s.refc['U']=18  # U1-U18 are placed by hand above
g=Grid(s,860,40,17,30,990)
g.D('BAV19W','RUN_P','RUN'); g.R('10k','PORT_FLT_N','+3V3_PI'); g.F('RUN_P')
for k in range(6):
    K=k+1; t=f'L{K}'; y=95+k*105
    u=s.nref('U'); q=s.nref('Q'); refs=[u,q]
    nic(s,LIBNAME+':TPS48111-Q1',u,'TPS48111AQDGXRQ1',(880,y),'marengo_revb:TPS48100_DGX_VSSOP-19',{'1':f'EN_{t}','2':'GND','3':f'INP_{t}','4':None,'5':'PORT_FLT_N','6':'GND','7':'GND','8':f'IWRN_{t}','9':f'TMR_{t}','10':'GND','11':None,'12':f'BST_{t}','13':f'V_{t}','14':f'GPD_{t}','15':f'GPU_{t}','17':f'SH{t}_N','18':f'CSP_{t}','19':f'ISCP_{t}','20':f'SH{t}_P'},ds='https://www.ti.com/lit/ds/symlink/tps4811-q1.pdf')
    nic(s,Qb,q,'IPB017N10N5LFATMA1',(950,y),'Package_TO_SOT_SMD:TO-263-7_TabPin4',{'1':f'G_{t}','2':f'V_{t}','3':f'V_{t}','4':f'D_{t}','5':f'V_{t}','6':f'V_{t}','7':f'V_{t}'})
    g=Grid(s,985,y-30,15,30,1160)
    refs+=[g.R('100R',f'SH{t}_P',f'CSP_{t}'),g.R('3.4k 1%',f'ISCP_{t}',f'SH{t}_N'),g.R('26.1k 1%',f'IWRN_{t}','GND'),g.R('100k',f'TMR_{t}','GND')]
    g.C('1uF 16V X7R',f'TMR_{t}','GND',C603); refs.append(f"C{s.refc['C']}")
    g.C('220nF 25V X7R',f'BST_{t}',f'V_{t}',C603); refs.append(f"C{s.refc['C']}")
    g.C('100nF 100V X7R',f'SH{t}_P','GND',C805); refs.append(f"C{s.refc['C']}")
    g.C('2.2uF 100V X7R',f'D_{t}','GND',C1210); refs.append(f"C{s.refc['C']}")
    g.nl()
    refs+=[g.R('100k',f'GPU_{t}',f'G_{t}'),g.R('22R',f'GPD_{t}',f'G_{t}',R603),g.D('BAV19W',f'GC_{t}',f'G_{t}')]
    g.C('100nF 100V X7R',f'GC_{t}','GND',C805); refs.append(f"C{s.refc['C']}")
    refs+=[g.R('10M',f'GC_{t}','GND',R603),g.Z('BZT52C15',f'G_{t}',f'V_{t}'),g.R('22k','RUN_P',f'EN_{t}'),g.R('10k',f'EN_{t}','GND'),g.R('100k','RUN_P',f'INP_{t}'),g.D('SK810L-TP',f'V_{t}','GND',SMC)]
    if K in PORTDNP:
        for r in refs: dnp(r)
wnote('''Why: a hard limb short used to latch the whole bus (U1 trips in under 0.4 ms, long before a 20 A fuse melts). Each port now opens itself first.
Short circuit: RISCP 3.4k on 1.5 mOhm = 35-45 A (15.6 uA x (R + 464)), PD low within 1.6 us, then 22R to the gate: Miller about 0.16 us, so the drain-side di/dt stays a few A/ns against the 2.2 uF and the bus caps. tools/port_sim.py: for 50 nH to 1 uH of wiring, 0.5-1 uH pack lead, 0-50 A on the other limbs, RS1 peaks at 130 A or less against U8 158 A and U1 160 A minimum: the port always opens first. A short at the board pads (50 nH) puts about 1.1-1.4 kA through the FET for under 2 us (5-14 mJ; IPB017: 1024 A pulse, 979 mJ avalanche), which is why the port uses the bank FET. A short in the harness (200 nH or more) is 400-540 A.
Overload: RIWRN 26.1k = 30 A (11.9 x RSET / (RSNS x IOC)), CTMR 1 uF with 100k across it: about 16 ms, then latch-off. The 20 A pigtail fuse stays as wire protection and as backup for a shorted FET.
Latch clears only when EN drops: an E-stop cycle, then reset. FLT_I of every port pulls PORT_FLT_N (Pi header): the Pi names the port whose output is low.
Start: EN and INP come from RUN through one diode (a pin fault can't drive RUN). The bus starts from 0 under U1, so the port wakes at VS 3 V with BST up (0.22 uF, about 20 ms): the gate rises through 100k into 100 nF (diode-coupled, 10M bleed, so PD never sees it), about 1 kV/s at the output: 20 mF of drive capacitance draws about 20 A, under the 30 A overload level. After that the drive caps follow the bus.
Off (E-stop, trip): the body diode still returns regen from the limb to the bus and the clamp. SK810L on each output carries the wiring current when the FET opens. 15 V zener gate to source.''',(860,735),210)

# ---------- rev B5: Pi heartbeat watchdog (Opus round 5 M-1, Joey 2026-11-07) ----------
head('2a. Pi heartbeat watchdog: no heartbeat, no latch',(450,610))
s.refc['U']=max(s.refc['U'],24)
uw=s.nref('U')
mkic('TPS3431',[('1','VDD','power_in'),('2','CWD','passive'),('3','EN','input'),('4','GND','power_in'),('5','SET1','input'),('6','WDI','input'),('7','WDO','open_collector'),('8','ENOUT','open_collector'),('9','EP','power_in')],'Package_SON:VSON-8-1EP_3x3mm_P0.65mm_EP1.65x2.4mm','https://www.ti.com/lit/ds/symlink/tps3431.pdf','Programmable watchdog timer with enable, 1.8-6.5 V, open-drain WDO')
nic(s,LIBNAME+':TPS3431',uw,'TPS3431DRBR',(480,660),'Package_SON:VSON-8-1EP_3x3mm_P0.65mm_EP1.65x2.4mm',{'1':'VWD','2':'CWD','3':'VWD','4':'GND','5':'VWD','6':'WDI','7':'WDO_N','8':None,'9':'GND'},ds='https://www.ti.com/lit/ds/symlink/tps3431.pdf')
g=Grid(s,520,625,17,30,700)
g.R('2.2k','VCTL','VWD',R603); g.Z('BZT52C3V3','VWD','GND'); g.C('100nF 25V X7R','VWD','GND'); g.C('3.3nF C0G','CWD','GND',C603); g.F('VWD')
g.nl(); g.R('10k','WD_FEED','WDI'); g.R('100k','WDI','GND'); g.R('22k','VWD','WDO_N')
Q(NPN,'MMBT3904',(640,670),'WDO_N','GND','L_E')
wnote('''The Pi toggles WD_FEED from its control loop (software, not a PWM peripheral) every 50-100 ms; the watchdog wants a falling edge at WDI at least every 270 ms (CWD 3.3 nF C0G: 77.4 x C + 55 ms = 310 ms typ, 270-355 ms with the +-9.5 % timer and 5 % cap band). Miss it and WDO goes low for 200 ms: the NPN in Q5's emitter turns off, the seal-in breaks and RUN falls like a loop open. The latch stays off after WDO releases: only a reset press re-arms it.
Fail-safe: the NPN only conducts while the watchdog's own rail (3.3 V zener from VCTL, not the Pi) feeds its base through 22k and WDO is released. Pi dead, unpowered, rebooting, hung or its pin floating (100k pull-down): no edges, no latch. Watchdog rail lost: no base drive, no latch. It sits in series, so it can only drop the latch, never hold it (a shorted NPN just removes the watchdog).
While the Pi stays dead the watchdog repeats 310 ms released / 200 ms low; a reset press in a released window closes the latch until the next low, under 0.36 s, with no drive commands (the Pi is dead). The first 0.3 s after VCTL comes up the watchdog is released before its first timeout.
Software requirement: drive CanTimeout set and read back before every enable.''',(450,700),190)
# absorber cans 4-6, last so earlier refs keep their numbers: the APZ line is obsolete (Joey, 16:46Z)
g=Grid(s,38.1,900,25.4,30.48,200)
for _ in range(5): g.CP('47uF 80V EEH-ZC1K470P','ABS','ABS_N','Capacitor_SMD:CP_Elec_10x10.5')  # 8 cans in all: 75 V worst, 6 % under rating
# bank A drain ceramics (rev B5 thermal study): the bank turns off in 50-200 ns, so the copper between the absorber and
# the drain kicks the drain. 4.4 uF here holds BAT at 74 V and the drain under 85 V with a 2 nH absorber loop
# (THERMAL_OPTION.md); without them BAT reaches 78-111 V. Placed right at the drains, GND pads straight to L2.
g.C('4.7uF 100V X7R','SW_D','GND',C1210,k=2)
# pack-entry ceramics (layout stage 2): the routed absorber loop is 3-5 nH (SMC diode bodies, can ESL, the comb spines), not
# the 2 nH first budgeted. About 8.8 uF of ceramics at BAT (4 x 4.7 uF 100 V, about 2 uF each at 54 V bias) takes the fast
# edge: BAT 73.8 V and the bank A drain 80-82 V with a 5 nH loop (absorber_sim, LAYOUT_RULES).
g.C('4.7uF 100V X7R','BAT','GND',C1210,k=4)
writelib()
s.write([('/'+ROOT,'1')])
print('ok',{k:v for k,v in s.refc.items() if not k.startswith('#')})
