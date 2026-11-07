"""Rev B power board, one flat sheet. Edit this, not the .kicad_sch. Run tools/regen.sh."""
import os,sys
sys.path.insert(0,os.path.dirname(os.path.abspath(__file__)))
from kit import *
open(OUT+'/sym-lib-table','w').write('(sym_lib_table\n\t(version 7)\n\t(lib (name "marengo_revb")(type "KiCad")(uri "${KIPRJMOD}/marengo_revb.kicad_sym")(options "")(descr "rev B project symbols"))\n)\n')
open(OUT+'/fp-lib-table','w').write('(fp_lib_table\n\t(version 7)\n\t(lib (name "marengo_revb")(type "KiCad")(uri "${KIPRJMOD}/marengo_revb.pretty")(options "")(descr "rev B project footprints"))\n)\n')
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
def Q(lid,val,at,b,e,c): r=s.nref('Q'); nic(s,lid,r,val,at,SOT23,{'1':b,'2':e,'3':c}); return r
NPN='Transistor_BJT:Q_NPN_BEC'; PNP='Transistor_BJT:Q_PNP_BEC'
mkic('IPB017N10N5LF',[('1','G','input'),('2','S','passive'),('3','S','passive'),('4','D','passive'),('5','S','passive'),('6','S','passive'),('7','S','passive')],'Package_TO_SOT_SMD:TO-263-7_TabPin4','https://www.infineon.com/dgdl/Infineon-IPB017N10N5LF-DataSheet-v02_02-EN.pdf','100 V 1.7 mOhm OptiMOS linear-mode FET, D2PAK-7',ref='Q')
mkic('IQB150W8-12',[('1','+VIN','power_in'),('2','R.C','passive'),('3','-VIN','power_in'),('5','RS-','input'),('6','TRIM','input'),('7','RS+','input'),('8','+VOUT','passive'),('4','-VOUT','passive')],'marengo_revb:IQB150W8','https://www.meanwell.com/Upload/PDF/IQB150W8/IQB150W8-SPEC.PDF','Mean Well 150 W quarter brick, 9-75 V in, 12 V 12.5 A out')
TLV=custom('TLV1805','tlv1805',ds='https://www.ti.com/lit/ds/symlink/tlv1805-q1.pdf')
LDO=custom('TPS7A4001','tps7a4001',ds='https://www.ti.com/lit/ds/symlink/tps7a40.pdf',renum={'EP':'9'})

# ---------- battery in and E-stop bank ----------
head('1. Battery in and E-stop switch (LM5069-1, two FETs)',(25.4,22))
term('M4 terminal, BMS P+',(40.64,45.72),'BAT')
term('M4 terminal, BMS P-',(40.64,68.58),'GND')
g=Grid(s,68.58,38.1,20.32,30.48,150)
g.Z('SMDJ58A','BAT','GND',SMC); g.C('1uF 100V','BAT','GND',C1210); g.C('100nF 100V','SH_P','GND',C805); g.F('BAT'); g.F('GND'); g.F('SH_P')
nic(s,'Device:R_Shunt','RS1','0.5mR CSS4J-4026R-L500F 5W',(177.8,48.26),SH,{'1':'BAT','2':'SH_P','3':'SH_N','4':'SW_D'})
Qb='marengo_revb:IPB017N10N5LF'
nic(s,Qb,'Q1','IPB017N10N5LFATMA1',(228.6,48.26),'Package_TO_SOT_SMD:TO-263-7_TabPin4',{'1':'G1','2':'VBUS','3':'VBUS','4':'SW_D','5':'VBUS','6':'VBUS','7':'VBUS'})
nic(s,Qb,'Q2','IPB017N10N5LFATMA1',(292.1,48.26),'Package_TO_SOT_SMD:TO-263-7_TabPin4',{'1':'G2','2':'VBUS','3':'VBUS','4':'SW_D','5':'VBUS','6':'VBUS','7':'VBUS'})
g=Grid(s,335.28,38.1,20.32,30.48,420)
g.R('10R','GATE','G1'); g.R('10R','GATE','G2')
nic(s,'Power_Management:LM5069MM-1','U1','LM5069MMX-1',(63.5,114.3),'Package_SO:MSOP-10_3x3mm_P0.5mm',{'1':'SH_N','2':'SH_P','3':'UVLO','4':'GND','5':'GND','6':'TIMER','7':'PWR','8':'PGD','9':'VBUS','10':'GATE'},ds='https://www.ti.com/lit/ds/symlink/lm5069.pdf')
g=Grid(s,114.3,96.52,20.32,30.48,420)
g.R('34k 1%','PWR','GND'); g.C('22nF C0G','TIMER','GND',C603); g.D('BAV19W','CDV','GATE'); g.C('100nF 100V C0G','CDV','GND',C1210); g.R('1k','GATE','CDV_B')
g.R('33k','RUN','UVLO'); g.R('33k','UVLO','GND')
Q(PNP,'MMBTA92',(275,111.76),'CDV_B','CDV','GND')
note('''The bank is the soft start for everything: BMS sees no inrush. Reset closes it, loop open drops it. No software in the path.
Rs 0.5 mOhm: limit 97-123 A (VCL 48.5-61.5 mV), breaker 160-260 A (VCB 80-130 mV). Above peak draw and JK DOC 80 A. 1.8 W at 60 A.
Start: Cdv 100 nF, gate 10-22 uA, 100-220 V/s, 0.25-0.55 s to full. Bus ceiling 9.4 mF at 2.06 A hot DC SOA line (112 W, 45 K Tj rise). Drive caps unmeasured.
Power limit 544 W, RPWR = 125k x Rs x P, 5 mV floor per SNVS452G sec 9.2.1.2.3. Timer 22 nF: 1.0 ms typ, 1.8 ms max.
Fault check: 675 W (+24 %) is 12.4 A at 54.6 V against about 15 A at 1.8 ms, 70 C, one FET, scaled from the 10 ms curve as sqrt(t). Read the real curve at layout.
FETs: 0.85 mOhm for the pair, 5.2 W hot at 60 A, 1.3 W at 30 A. UVLO low sinks the gate at 2 mA: about 170 us off.
Body diodes pass regen back to the pack with the bank open. OVLO tied low: opening on overvoltage would strand regen on the bus.
LM5069-1 latches on a fault. Clear: E-stop cycle, then reset. PGD (open drain) holds the brick off until the bus is up, per TI's keep-the-load-off note.
SMDJ58A: catches the battery lead's inductive kick when the bank opens. Clears 54.6 V, FETs and U1 are 100 V.''',(300,92))

# ---------- control rail and latch ----------
head('2. Control rail and E-stop latch (no logic IC)',(25.4,172))
nic(s,LDO,'U2','TPS7A4001DGNR',(63.5,200),'Package_SO:HVSSOP-8-1EP_3x3mm_P0.65mm_EP1.57x1.89mm',{'1':'VCTL','2':'FB_CTL','3':None,'4':'GND','5':'BAT','6':None,'7':None,'8':'BAT','9':'GND'})
g=Grid(s,114.3,185.42,20.32,30.48,200)
g.C('4.7uF 25V','VCTL','GND',C805); g.R('464k','VCTL','FB_CTL'); g.R('49.9k','FB_CTL','GND'); tp(g,'VCTL')
note('12.1 V, 11 mA. On the battery side, so regen through the body diodes keeps it up with the BMS open. Runs the latch and the clamp. 0.5 W at 54.6 V.',(114.3,232))
conn(5,'E-stop loop: 1 out, 2 GND, 3 return, 4-5 reset NO',(241.3,200),GH(5),['LOOP_OUT','GND','LOOP_RET','RST_SRC','RST_IN'],ref='J20')
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
Brake resistor NC thermostat: wire it in series in the loop harness, no board parts. E-stop model still to confirm.''',(284.48,240))

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
nic(s,'marengo_revb:IQB150W8-12','U5','IQB150W8-12',(66,385),'marengo_revb:IQB150W8',{'1':'VBUS','2':'PGD','3':'GND','5':'GND','6':None,'7':'+12V_RAW','8':'+12V_RAW','4':'GND'})
g=Grid(s,114.3,367,20.32,30.48,200)
g.C('2.2uF 100V','VBUS','GND',C1210,k=2); g.C('22uF 25V','+12V_RAW','GND',C1210,k=2); g.F('+12V_RAW')
nic(s,'Device:R_Shunt','RS8','2mR CSS4J-4026K-2L00F 6W',(218,378),SH,{'1':'+12V_RAW','2':'SH12_P','3':'SH12_N','4':'+12V'})
nic(s,'Sensor_Energy:INA228','U6','INA228AIDGSR',(272,378),'Package_SO:TSSOP-10_3x3mm_P0.5mm',{'1':'+3V3_PI','2':'I2C_SDA','3':None,'4':'I2C_SDA','5':'I2C_SCL','6':'+3V3_PI','7':'GND','8':'+12V','9':'SH12_N','10':'SH12_P'},ds='https://www.ti.com/lit/ds/symlink/ina228.pdf')
g=Grid(s,305,367,20.32,30.48,330); g.C('100nF','+3V3_PI','GND'); tp(g,'+12V')
for nm,x in (('Jetson',360),('Pi 5 V buck',400),('fans',440)):
    conn(2,'12 V to '+nm,(x,378),MF2,['+12V','GND'])
conn(4,'Pi: 3V3, SDA, SCL, GND',(490,378),GH(4),['+3V3_PI','I2C_SDA','I2C_SCL','GND'],ref='J21')
g=Grid(s,515,367,20.32,30.48,530); g.F('+3V3_PI')
note('''On the bus, so the brick drops with the E-stop and the Pi goes with it. PGD on R.C keeps it off during the ramp: it starts at 9 V and would load the FETs mid-ramp.
-Vout is tied to GND here: the robot has one ground, P-. RS+ and RS- tied at the pins, Trim open. Input caps at the pins; the EMC filter is left out.
150 W at 12 V, 89 %: 18 W at full load, about 6 W at 50 W. Natural-air derating starts near 30 C. 12.7 mm tall: keep clear of the fan gap.
INA228 addresses: L arm 0x40, R arm 0x41, waist 0x42, aux 0x43, L leg 0x44, R leg 0x45, 12 V 0x46. Powered by the Pi's 3.3 V; Pi pull-ups on I2C.''',(25.4,404))

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
writelib()
s.write([('/'+ROOT,'1')])
print('ok',{k:v for k,v in s.refc.items() if not k.startswith('#')})
