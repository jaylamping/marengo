"""HP main sheets, LP digital sheets, stub boards. Datasheet refs live in RATIONALE-log.md."""
from sheets import *
import sheets as SH
FET='Transistor_FET:IPT015N10N5'; FETFP='Package_TO_SOT_SMD:Infineon_PG-HSOF-8-1'
R1206='Resistor_SMD:R_1206_3216Metric'; R2512='Resistor_SMD:R_2512_6332Metric'; SMC='Diode_SMD:D_SMC'; SOT23='Package_TO_SOT_SMD:SOT-23'
REDCUBE='TerminalBlock_Wuerth:Wuerth_REDCUBE-THR_WP-THRSH_74651194_THR'
TPFP='TestPoint:TestPoint_Keystone_5015_Micro_Mini'
GH=lambda n:f'Connector_JST:JST_GH_SM0{n}B-GHS-TB_1x0{n}-1MP_P1.25mm_Horizontal'
MF2='Connector_Molex:Molex_Micro-Fit_3.0_43045-0212_2x01_P3.00mm_Vertical'
BAR='Package_TO_SOT_SMD:TO-263-2'
HDR2='Connector_PinHeader_2.54mm:PinHeader_1x02_P2.54mm_Vertical'
FB402='Inductor_SMD:L_0402_1005Metric'; FB603='Inductor_SMD:L_0603_1608Metric'
B2BFP='Connector_Molex_Milligrid:Molex_8783230xx_2x15_P2.0mm_Header_Vertical_Polarized'
def reg():
    GLOBALS.update('''VSYS MID VMOT VCLAMP12 +3V3_HP +3V3_AON I2C_HP_SDA I2C_HP_SCL I2C_AFE_SDA I2C_AFE_SCL AFE_ALERT AFE_DFETOFF_SUP AFE_RST_SHUT TS2_WAKE
      ESTOP_EN_A ESTOP_EN_B BANK_B_CLOSE FLT_A MID_SENSE VMOT_SENSE CLAMP_STATUS CHARGER_PRESENT NTC_BOARD1 NTC_BRAKE NTC_INTAKE NTC_EXHAUST
      USB_SUP_DM USB_SUP_DP USB_A_DM USB_A_DP USB_B_DM USB_B_DP HUB_RST_N FAN1_PWM FAN2_PWM FAN3_PWM FAN1_TACH FAN2_TACH FAN3_TACH
      LED_R LED_G LED_B BUZZER CHARGE_LED POWER_BTN BENCH_JUMPER SUP_OPEN_A SUP_OPEN_B LOOP_SENSE_A LOOP_SENSE_B QWIIC_SDA QWIIC_SCL
      EN_CLEAN5 EN_CLEAN3 RAW_5V +12V +24V BUCK12_OFF BUCK24_OFF PG_12V PG_24V RAIL_FLT RAIL_ALERT_A RAIL_ALERT_B I2C_A_SDA I2C_A_SCL I2C_B_SDA I2C_B_SCL'''.split())
    GLOBALS.update('EN_24V EN_12V_JETSON EN_12V_THIRD EN_12V_AUX EN_12V_PI EN_7V4 EN_5V EN_3V3 EN_SPARE'.split())
    for k in range(1,7): GLOBALS.update({f'PORT_EN{k}',f'PORT_REQ{k}',f'IMON{k}',f'CAN{k}_H',f'CAN{k}_L',f'CAN{k}_TXD',f'CAN{k}_RXD'})
    GLOBALS.update('L12_OUT EN_12V_THIRD_M EN_12V_AUX_M EN_SPARE_M WD_KICK WD_N WD_N_M ESTOP_ARMED'.split())
def dnp(s,ref):
    for it in s.items:
        if it[0]=='symbol' and it[-1]==['__inst__',ref]:
            for x in it:
                if isinstance(x,list) and x and x[0]=='dnp': x[1]='yes'
            return
    raise KeyError(ref)
def _F(g,val,a,b,fp): return g.two('Device:Fuse','F',val,a,b,fp)
def _FB(g,val,a,b,fp=FB402): return g.two('Device:FerriteBead','FB',val,a,b,fp)
def _TV(g,val,a,b,fp): return g.two('Device:D_TVS','D',val,a,b,fp,rot=270)
def _RT(g,val,a,b,fp=''): return g.two('Device:Thermistor_NTC','RT',val,a,b,fp)
def _PF(g,val,a,b,fp): return g.two('Device:Polyfuse','F',val,a,b,fp)
def _TP(g,net):
    s=g.s; p=g.at(); nic(s,'Connector:TestPoint',s.nref('TP'),'TP',p,TPFP,{'1':net})
Grid.Fu=_F; Grid.FB=_FB; Grid.TV=_TV; Grid.RT=_RT; Grid.PF=_PF; Grid.TP=_TP
def fet(s,val,at,g,d,so,lid=FET,fp=FETFP,ref=None):
    r=ref or s.nref('Q'); nic(s,lid,r,val,at,fp,{'1':g,'2':so,'3':d}); return r
def sot(s,lid,val,at,g,d,so,fp=SOT23):
    r=s.nref('Q'); nic(s,lid,r,val,at,fp,{'1':g,'2':so,'3':d}); return r
def conn(s,n,val,at,fp,nets):
    r=s.nref('J'); nic(s,f'Connector_Generic:Conn_01x{n:02d}',r,val,at,fp,{str(i+1):v for i,v in enumerate(nets)}); return r
def ipb():
    if 'IPB017N10N5LF' not in G()['CUSTOM']:
        G()['mkic']('IPB017N10N5LF',[('1','G','input'),('2','S','passive'),('3','S','passive'),('4','D','passive'),('5','S','passive'),('6','S','passive'),('7','S','passive')],'Package_TO_SOT_SMD:TO-263-7_TabPin4','https://www.infineon.com/dgdl/Infineon-IPB017N10N5LF-DataSheet-v02_02-EN.pdf','100 V 1.7 mOhm OptiMOS linear-mode FET, D2PAK-7',ref='Q')
    return 'marengo:IPB017N10N5LF'
def dmp():
    if 'DMP10H400SE' not in G()['CUSTOM']:
        G()['mkic']('DMP10H400SE',[('1','G','input'),('2','D','passive'),('3','S','passive')],'Package_TO_SOT_SMD:SOT-223-3_TabPin2','','Diodes Inc 100 V P-FET SOT-223. Pin map inferred from convention, verify.',ref='Q')
    return 'marengo:DMP10H400SE'
def G(): return SH.G
def pwr263s():
    if 'PWR263S' not in G()['CUSTOM']:
        G()['mkic']('PWR263S',[('1','A','passive'),('3','B','passive'),('2','TAB','passive')],BAR,'https://www.bourns.com/docs/product-datasheets/pwr263s-35.pdf','Bourns PWR263S-35 35 W D2PAK resistor. 2 leads on pads 1, 3. Tab pad 2 is isolated (Bourns: resistor electrically isolated from the backplate), tied to GND for copper.',ref='R')
    return 'marengo:PWR263S'
def v10pm15():
    if 'V10PM15' not in G()['CUSTOM']:
        G()['mkic']('V10PM15',[('1','A1','passive'),('2','A2','passive'),('3','K','passive')],'Package_TO_SOT_SMD:TO-277A','https://www.vishay.com/docs/87432/v10pm15.pdf','Vishay 150 V 10 A TMBS Schottky, SMPC TO-277A. Pads 1, 2 anode, tab cathode.',ref='D')
    return 'marengo:V10PM15'
# ---------- estop banks ----------
def estop():
    s=base('estop_banks','estop_banks: E-stop banks A and B, VMOT precharge',300,'TPS48100-Q1 simple-driver mode (SLVSGC0 Fig 7-11). No firmware in the enable path.')
    d=custom('TPS48100-Q1','tps48100-q1',fp='',ds='https://www.ti.com/lit/ds/symlink/tps4810-q1.pdf',et={'14':'passive','15':'passive'})
    def drv(x,y,ref,t,en,inp1,inp2,src,g2,csn,csp,iscp,tmr):
        nic(s,d,ref,'TPS48100QDGXRQ1',(x+33.02,y+33.02),'marengo:TPS48100_DGX_VSSOP-19',{'1':en,'2':inp2,'3':inp1,'4':None,'5':'FLT'+t,'6':'GND','7':'GND','8':iscp,'9':tmr,'10':'GND','11':g2,'12':'BST'+t,'13':src,'14':'GATE'+t,'15':'GATE'+t,'17':csn,'18':csp,'19':None,'20':'VS'+t})
        g=Grid(s,x+88.9,y+7.62,13.97,27.94,x+170)
        g.R('100R','VSYS','VS'+t); g.C('100nF 100V','VS'+t,'GND',C805); g.C('1uF 25V X7R','BST'+t,src,C603); g.R('100k',en,'GND'); g.R('100k','FLT'+t,'+3V3_HP'); g.R('1M','GATE'+t,src); g.TP('FLT'+t); g.F('VS'+t)
    drv(25.4,30.48,'U301','_A','ESTOP_EN_A','ESTOP_EN_A','GND','MID',None,'VSYS_SENS_N','VSYS_SENS_P','ISCP_A','TMR_A')
    g=Grid(s,114.3,88.9,13.97,27.94,200); g.R('10.5k 1%','ISCP_A','GND'); g.R('100k','TMR_A','GND'); g.C('680pF C0G','TMR_A','GND')
    drv(25.4,119.38,'U302','_B','ESTOP_EN_B','BANK_B_CLOSE','ESTOP_EN_B','VMOT','G2_B','VS_B','VS_B','GND','GND')
    g=Grid(s,114.3,177.8,13.97,27.94,200); g.R('100k','BANK_B_CLOSE','GND')
    for k in range(3):
        x=215.9+k*40.64
        fet(s,'IPT015N10N5',(x,55.88),f'GA{k}','VSYS','MID'); fet(s,'IPT015N10N5',(x,144.78),f'GB{k}','MID','VMOT')
    g=Grid(s,215.9,88.9,17.78,30.48,330)
    for k in range(3): g.R('2.2R','GATE_A',f'GA{k}')
    g.nl(180)
    for k in range(3): g.R('2.2R','GATE_B',f'GB{k}')
    s.text('Bank A: VSYS to MID',(215.9,33.02),size=1.5,bold=True); s.text('Bank B: MID to VMOT',(215.9,121.92),size=1.5,bold=True)
    s.text('Drain toward VSYS, source toward VMOT: body diodes conduct VMOT to VSYS, so regen still reaches the pack with the banks open (PLAN 3.4).',(215.9,36.83))
    # precharge
    s.text('VMOT precharge (IPT015N10N5 SOA is 0.7 A at 54 V, 10 ms, so a hard close on 1.8 mF is out). U302 EN and INP2 on ESTOP_EN_B: loop closed and latch armed, G2 precharges at once, bank B stays open.\nBANK_B_CLOSE (INP1, 100k down) closes bank B on command once VMOT reads within 1.5 V of VSYS. Up to 9 mA of VMOT standing load sets a 0.9 V floor across R318, so 0.5 V is out of reach. 0.1% dividers: true gap 1.2 to 1.8 V. 0.81 s, 1.12 s worst case, firmware timeout 1.5 s, then open the loop. EN low kills G1 and G2. Energy 2.68 J, tau 180 ms, peak 29.8 W.',(25.4,198.12),size=1.5)
    fet(s,'IPT015N10N5',(55.88,241.3),'G2_G','PRE_D','VMOT')
    g=Grid(s,91.44,233.68,17.78,30.48,330)
    nic(s,pwr263s(),s.nref('R'),'100R PWR263S-35-1000F',(76.2,241.3),BAR,{'1':'MID','2':'GND','3':'PRE_D'}); g.x+=17.78
    g.R('10R','G2_B','G2_G'); g.R('1M','G2_G','VMOT'); g.C('100nF 100V','MID','GND',C805)
    g.R('1M 0.1%','MID','MID_DIV',R1206); g.R('56k 0.1%','MID_DIV','GND'); g.C('1nF','MID_DIV','GND'); g.R('49.9R','MID_DIV','MID_SENSE')
    g.R('1M 0.1%','VMOT','VMOT_DIV',R1206); g.R('56k 0.1%','VMOT_DIV','GND'); g.C('1nF','VMOT_DIV','GND'); g.R('49.9R','VMOT_DIV','VMOT_SENSE')
    g.nl(); g.TP('VSYS'); g.TP('MID'); g.TP('VMOT')
    s.text('Status (TPS48100 has no VDS output): MID, VMOT dividers 1M / 56k, 3.3 V = 62 V, plus VSYS on LP. Bank A short reads MID near VSYS with both open and VMOT low.\n\nBank A short protection: ISCP 10.5k = 40 mV (31.6-49.5 mV) on RS201 0.3 mOhm = 105-165 A, under BMS SCD and over OCD1 max 93 A. About 10 us, latch-off (100k + 680 pF across TMR), FLT_A to the supervisor. Clears on an E-stop cycle. Keeps compute up on a VMOT short.',(215.9,212))
    return s
# ---------- VMOT bulk and clamp ----------
def vmot():
    s=base('vmot_bulk_clamp','vmot_bulk_clamp: VMOT bulk, TVS, self-powered regen clamp',400,'Regen clamp powers itself from VMOT, works with supervisor, BMS and pack all off (PLAN 3.4).')
    s.paper='A2'
    g=Grid(s,25.4,35.56,38.1,30.48,350)
    for k in range(18): g.CP('100uF 80V HHXF800GRA101MJH0G','VMOT','GND','Capacitor_SMD:CP_Elec_10x14.3')
    g.Z('SMDJ58A','VMOT','GND',SMC); g.Z('SMDJ58A','VMOT','GND',SMC); g.F('VMOT')
    s.text('Bulk: 18 x 100 uF 80 V hybrid, 1.8 mF. 220 uF 80 V 10x12.5 does not exist. Can is 10 x 16.5 mm: footprint is the closest library match, real land owed.',(25.4,15.24),size=1.5)
    y=127
    a=custom('TPS7A4001','tps7a4001',ds='https://www.ti.com/lit/ds/symlink/tps7a40.pdf',renum={'EP':'9'})
    nic(s,a,'U401','TPS7A4001DGNR',(55.88,y+22.86),'Package_SO:HVSSOP-8-1EP_3x3mm_P0.65mm_EP1.57x1.89mm',{'1':'VCLAMP12','2':'FB12','3':None,'4':'GND','5':'VMOT','6':None,'7':None,'8':'VMOT','9':'GND'})
    g=Grid(s,114.3,y,15.24,30.48,380)
    g.C('1uF 100V','VMOT','GND',C1210); g.C('4.7uF 25V','VCLAMP12','GND',C805); g.R('464k','VCLAMP12','FB12'); g.R('49.9k','FB12','GND')
    g.R('9.09k','VCLAMP12','VREF_CL'); 
    nic(s,'Reference_Voltage:LM4040DBZ-2.5','U403','LM4040AIM3-2.5',(215.9,y+15.24),SOT23,{'1':'VREF_CL','2':'GND','3':None})
    y2=y+63.5
    t=custom('TLV1805','tlv1805',ds='https://www.ti.com/lit/ds/symlink/tlv1805-q1.pdf')
    nic(s,t,'U402','TLV1805DBVR',(55.88,y2+17.78),'Package_TO_SOT_SMD:SOT-23-6',{'1':'CL_OUT','2':'GND','3':'VREF_CL','4':'CL_SENSE','5':'GND','6':'VCLAMP12'})
    g=Grid(s,114.3,y2,15.24,30.48,380)
    g.R('499k 0.1%','VMOT','CL_DIV',R1206); g.R('604k 0.1%','CL_DIV','CL_SENSE',R1206); g.R('51.1k 0.1%','CL_SENSE','GND'); g.R('19.1M','CL_OUT','CL_SENSE'); g.C('100pF C0G','CL_SENSE','GND'); g.C('100nF','VCLAMP12','GND')
    u=custom('UCC27517','ucc27517',ds='https://www.ti.com/lit/ds/symlink/ucc27517.pdf')
    y3=y2+55.88
    nic(s,u,'U404','UCC27517DBVR',(55.88,y3+17.78),'Package_TO_SOT_SMD:SOT-23-5',{'1':'VCLAMP12','2':'GND','3':'CL_OUT','4':'GND','5':'CL_GDRV'})
    g=Grid(s,114.3,y3,15.24,30.48,380)
    g.C('1uF 25V','VCLAMP12','GND',C603); g.R('4.7R','CL_GDRV','CL_G'); g.R('100k','CL_G','GND')
    fet(s,'IPT015N10N5',(203.2,y3+15.24),'CL_G','CL_D','CL_S')
    r=s.nref('RS'); nic(s,'Device:R_Shunt',r,'2mR CSS4J-4026K-2L00F 6W',(241.3,y3+15.24),'Resistor_SMD:R_Shunt_Isabellenhuette_BVR4026',{'1':'CL_S','2':'CL_SP','3':'CL_SN','4':'GND'})
    g=Grid(s,279.4,y3,15.24,30.48,520)
    nic(s,'Device:D_Schottky_AKA',s.nref('D'),'MBRB30H100CT-E3/81',(287,y3+6),'Package_TO_SOT_SMD:TO-263-3_TabPin2',{'1':'CL_D','2':'VMOT','3':'CL_D'}); g.x+=30; g.R('10R','CL_SP','CL_IP'); g.R('10R','CL_SN','CL_IN'); g.C('100nF','CL_IP','CL_IN'); g.R('100R','VMOT','CL_VB'); g.C('100nF','+3V3_HP','GND')
    nic(s,'Sensor_Energy:INA228','U405','INA228AIDGSR',(444.5,y3+15.24),'Package_SO:TSSOP-10_3x3mm_P0.5mm',{'1':'I2C_HP_SCL','2':'+3V3_HP','3':None,'4':'I2C_HP_SDA','5':'I2C_HP_SCL','6':'+3V3_HP','7':'GND','8':'CL_VB','9':'CL_IN','10':'CL_IP'},ds='https://www.ti.com/lit/ds/symlink/ina228.pdf')
    sot(s,'Transistor_FET:Q_NMOS_GSD','BSS123',(330.2,y2+15.24),'CL_OUT','CLAMP_STATUS','GND')
    conn(s,1,'M4 terminal, Arcol HS25 10R +',(381,y2+7.62),REDCUBE,['VMOT'])
    conn(s,1,'M4 terminal, Arcol HS25 10R -',(381,y2+22.86),REDCUBE,['CL_D'])
    conn(s,2,'brake R NTC',(431.8,y2+15.24),GH(2),['NTC_BRAKE','GND'])
    s.text('Clamp: 56.77 V on, 55.77 V off (VOH 11.7 V, 19.1M hysteresis). Spread 56.08-57.46 V full temperature. Arcol HS25 10 Ohm 25 W clad, external on 2 mm plate: 322 W, 5.7 A.\nFET, 2 mOhm shunt (INA228 SHUNT_CAL 2000, ADCRANGE 0), 30 A Schottky and M4 terminals are sized for the leg stage 2 Ohm resistor, 29 A, 1.7 kW short time: only the\nexternal resistor swaps.\nINA228 0x4D: clamp power and energy on its own accumulator. Budget about 5 mA, 0.23 W in the LDO.\nFailure: stuck-on 322 W: NC thermostat on the HS25 plate sits in the E-stop loop, so it opens the banks in hardware, supervisor hung or not. NTC warns first. SMDJ58A stays off under 64.4 V, spike protection only.\nRegen decision (coordinator 11-06): 10 Ohm in rev A. 30 A regen against 5.75 A clamp: BMS opening mid-brake rises 13.5 V/ms, drives stop at 60 V in about 185 us, torque drops. Leg stage fits 2 Ohm.',(279.4,y-2.54),size=1.5)
    return s
# ---------- port channel ----------
PORT_PINS=['PORT_EN','IMON','CANH','CANL','A1','A0','V_PORT']
def port():
    s=base('port_channel','port_channel: one limb port (instanced x6, legs DNP)',500,'LM5069-1 latched hot swap, INA228 + INA241 on one shunt, blade fuse, load-side mini clamp.')
    s.paper='A2'; s.multi=100; s.hiers={k:'passive' for k in PORT_PINS}; s.refc['Q']=502
    y=30.48
    nic(s,'Device:R_Shunt','RS501','3mR WSLP39213L000FEA 5W',(50.8,y+12.7),'marengo:WSLP3921_Kelvin',{'1':'VMOT','2':'SH_P','3':'SH_N','4':'Q_D'})
    Q=ipb()
    nic(s,Q,'Q501','IPB017N10N5LFATMA1',(101.6,y+12.7),'Package_TO_SOT_SMD:TO-263-7_TabPin4',{'1':'G1','2':'V_OUT','3':'V_OUT','4':'Q_D','5':'V_OUT','6':'V_OUT','7':'V_OUT'})
    nic(s,Q,'Q502','IPB017N10N5LFATMA1',(165.1,y+12.7),'Package_TO_SOT_SMD:TO-263-7_TabPin4',{'1':'G2','2':'V_OUT','3':'V_OUT','4':'Q_D','5':'V_OUT','6':'V_OUT','7':'V_OUT'}); dnp(s,'Q502')
    g=Grid(s,215.9,y+2.54,15.24,30.48,540)
    g.R('10R','GATE','G1'); g.R('10R','GATE','G2'); g.D('MBRS3100T3G','V_OUT','GND',SMC); g.C('1uF 100V','VMOT','GND',C1210); g.C('100nF 100V','SH_P','GND',C805)
    g.Fu('20A 58V 0997020.WXN','V_OUT','V_PORT','Fuse:Fuseholder_Blade_Mini_Keystone_3568')
    g.Fu('leg fuse, format open','V_OUT','V_PORT','Fuse:Fuseholder_Blade_ATO_Littelfuse_Pudenz_2_Pin'); dnp(s,f"F{s.refc['F']}")
    g.TV('SMAJ58CA','CANH','GND','Diode_SMD:D_SMA'); g.TV('SMAJ58CA','CANL','GND','Diode_SMD:D_SMA'); g.PF('0.14A MF-MSMF014/60X','CAN_REF','GND','Fuse:Fuse_1812_4532Metric'); g.TP('V_PORT'); g.F('SH_P')
    conn(s,1,'M4 terminal, port +',(508,y+7.62),REDCUBE,['V_PORT']); conn(s,1,'M4 terminal, port -',(508,y+22.86),REDCUBE,['GND'])
    conn(s,3,'CAN to trunk',(551,y+12.7),GH(3),['CANH','CANL','CAN_REF'])
    y=78.74
    nic(s,'Power_Management:LM5069MM-1','U501','LM5069MMX-1',(63.5,y+22.86),'Package_SO:MSOP-10_3x3mm_P0.5mm',{'1':'SH_N','2':'SH_P','3':'UVLO','4':'OVLO','5':'GND','6':'TIMER','7':'PWR','8':None,'9':'V_OUT','10':'GATE'},ds='https://www.ti.com/lit/ds/symlink/lm5069.pdf')
    g=Grid(s,114.3,y+2.54,15.24,30.48,540)
    g.R('191k','VMOT','UVLO',R1206); g.R('18.7k','UVLO','GND'); g.R('95.3k','VMOT','OVLO',R1206); g.R('4.12k','OVLO','GND'); g.R('69.8k','PWR','GND'); g.C('100nF C0G','TIMER','GND',C603)
    g.D('BAV19W','CDV','GATE'); g.C('47nF 100V C0G','CDV','GND',C1206); g.R('1k','GATE','CDV_B')
    sot(s,'Transistor_FET:Q_NMOS_GSD','BSS123',(279.4,y+15.24),'EN_INV','UVLO','GND')
    sot(s,'Transistor_FET:Q_NMOS_GSD','BSS123',(330.2,y+15.24),'PORT_EN','EN_INV','EN_S')
    r=s.nref('Q'); nic(s,'Transistor_BJT:Q_PNP_BEC',r,'MMBTA92 PNP',(381,y+15.24),SOT23,{'1':'CDV_B','2':'CDV','3':'GND'})
    g=Grid(s,419.1,y+2.54,15.24,30.48,540)
    g.R('1M','VMOT','EN_INV',R1206); g.Z('MMSZ5242B 12V','EN_INV','GND'); g.R('100k','PORT_EN','GND'); g.R('1M','VCLAMP12','GND')
    sot(s,'Transistor_FET:Q_NMOS_GSD','BSS123',(457.2,y+45.72),'VCLAMP12','EN_S','GND')
    y=139.7
    nic(s,'Sensor_Energy:INA228','U502','INA228AIDGSR',(63.5,y+20.32),'Package_SO:TSSOP-10_3x3mm_P0.5mm',{'1':'A1','2':'A0','3':None,'4':'I2C_HP_SDA','5':'I2C_HP_SCL','6':'+3V3_HP','7':'GND','8':'VB_F','9':'IN_N','10':'IN_P'},ds='https://www.ti.com/lit/ds/symlink/ina228.pdf')
    g=Grid(s,114.3,y+2.54,15.24,30.48,540)
    g.R('10R','SH_P','IN_P'); g.R('10R','SH_N','IN_N'); g.C('100nF','IN_P','IN_N'); g.R('100R','V_PORT','VB_F'); g.C('100nF','+3V3_HP','GND')
    nic(s,'Amplifier_Current:INA241A1xDDF','U503','INA241A1IDDFR',(228.6,y+20.32),'Package_TO_SOT_SMD:SOT-23-8',{'1':'SH_N','2':'GND','3':'GND','4':'GND','5':'IM_RAW','6':'+3V3_HP','7':'+3V3_HP','8':'SH_P'},ds='https://www.ti.com/lit/ds/symlink/ina241a.pdf')
    g=Grid(s,279.4,y+2.54,15.24,30.48,540)
    g.R('49.9R','IM_RAW','IMON'); g.C('100nF','+3V3_HP','GND')
    s.text('INA241: 1.65 V + 10 x Vsense, 30 mV/A at 3 mOhm, +/-48 A, sees regen. Ratiometric to +3V3_HP. Supervisor software trip reads this (1-15 A, default 15 A, aux 10 A), drops PORT_EN.\nINA228 ADCRANGE 0 (61.5 mV max VCL exceeds 40.96 mV range). SHUNT_CAL 4500, 60 A LSB. 100 Hz.',(362,y+5.08),size=1.5)
    y=203.2
    s.text('Load-side mini clamp, 59.42 V on, 58.43 V off (TL431 2.495 V), peak 0.39 A, 23 W. Covers a blown fuse with a backdriven arm (PLAN 3.4).',(25.4,y-5.08),size=1.5,bold=True)
    nic(s,'Reference_Voltage:TL431DBZ','U504','TL431BIDBZR',(50.8,y+15.24),SOT23,{'1':'TL_K','2':'TL_R','3':'GND'})
    sot(s,'Transistor_FET:Q_NMOS_GSD','BSS123',(101.6,y+15.24),'CAS_G','CAS_D','TL_K')
    r=s.nref('Q'); nic(s,dmp(),r,'DMP10H400SE-13',(152.4,y+15.24),'Package_TO_SOT_SMD:SOT-223-3_TabPin2',{'1':'PG','2':'PD','3':'V_PORT'})
    g=Grid(s,215.9,y+2.54,15.24,30.48,540)
    g.R('25.5k 0.1%','V_PORT','TL_M'); g.R('34.0k 0.1%','TL_M','TL_R'); g.R('2.61k 0.1%','TL_R','GND'); g.R('3.48M','PD','TL_R'); g.R('1M','V_PORT','CAS_G',R1206); g.Z('MMSZ5246B','CAS_G','GND'); g.R('33k','CAS_D','PG'); g.R('10k','V_PORT','PG'); g.Z('MMSZ5246B','V_PORT','PG')
    nic(s,pwr263s(),s.nref('R'),'150R PWR263S-35-1500F',(190.5,y+15.24),BAR,{'1':'PD','2':'GND','3':'GND'})
    s.text('Values shown: arm, waist, aux. LM5069 limit 16.2-20.5 A (48.5-61.5 mV / 3 mOhm), breaker 26.7-43.3 A, RPWR 69.8k = 200 W, fault timer 2.8-9.0 ms.\nReal overload limit is the supervisor software trip at 15 A: XT30 and enclosed 16 AWG are treated as 15 A continuous. The 20 A fuse is only the backstop for a shorted FET.\nLeg channels (DNP in rev A): Q502 fitted; RS501 1 mOhm (48.5-61.5 A); RPWR 23.7k; INA228 SHUNT_CAL rescaled. Leg fuse format open: DNP holder here beside F501, 997 MINI stops at 30 A.\nEnable: PORT_EN high (3V3, 100k down) turns Q3 on, Q2 off, UVLO released. EN_INV is pulled from VMOT itself with a 12 V zener, and Q3 sinks it through a FET gated by VCLAMP12, so a lost clamp supply turns the port off. PORT_EN comes through a tri-state buffer: E-stop or watchdog latch, or a dead supervisor, leaves Q2 on, UVLO low, port off. Latch reset: drop PORT_EN, wait 148 ms, raise it.\nUVLO on 32.0 V off 28.0 V, OVLO off 60.3 V. Starts on dV/dt: 47 nF covers 3.6 mF of drive capacitance.\nBus ground: non-isolated. CAN_REF ties to board GND through a 60 V PTC, so a lost motor return cannot pull drive current through the signal wire. TCAN1462V: pins +/-58 V, differential +/-45 V.\nSMAJ58CA stays off at 54.6 V sustained; it is for ESD and is not a +/-58 V clamp. Capacitance unchecked: scope on the bus.',(25.4,264.16),size=1.5)
    y=345
    s.text('CAN termination, split, at the connector. 54.6 V on H with L hard to GND: 24.8 W, 12.4 W each for up to about 2 s until the PTC trips (0.3 A trip, 0.5 A in 1.5 s max), 56R0 in the 35 W D2PAK. 56 + 56 + 0.7-15 = 113-127 Ohm. Jumper in for the end node.',(25.4,y-7.62),size=1.5,bold=True)
    nic(s,pwr263s(),s.nref('R'),'56R PWR263S-35-56R0F',(63.5,y+12.7),BAR,{'1':'CANH','2':'GND','3':'TERM_M'})
    nic(s,pwr263s(),s.nref('R'),'56R PWR263S-35-56R0F',(139.7,y+12.7),BAR,{'1':'TERM_M','2':'GND','3':'TERM_P'})
    g=Grid(s,215.9,y+2.54,15.24,30.48,540)
    g.C('4.7nF 100V','TERM_M','GND',C805); g.PF('0.10A MF-MSMF010/60X','TERM_P','TERM_J','Fuse:Fuse_1812_4532Metric')
    conn(s,2,'term jumper',(279.4,y+12.7),HDR2,['TERM_J','CANL'])
    return s
# ---------- supervisor ----------
def supervisor():
    s=base('supervisor_mcu','supervisor_mcu: STM32G0B1 supervisor, I2C mux',1400,'Supervisor can open the banks and ports, never close them against the loop. All enables fail off.')
    s.paper='A2'
    d=json.load(open(os.path.join(PARTS,'stm32g0b1.json')))
    REN={'USB_DM':'USB_SUP_DM','USB_DP':'USB_SUP_DP','SWDIO':'SUP_SWDIO','SWCLK':'SUP_SWCLK','NRST':'SUP_NRST','VSYS_SENSE':'ADC_VSYS','VMOT_SENSE':'ADC_VMOT',
      'NTC_BOARD1':'ADC_NTC1','NTC_BOARD2':'ADC_NTC2','NTC_BOARD3':'ADC_NTC3','NTC_BRAKE':'ADC_NTCB','ESTOP_A_STAT':'ADC_MID','ESTOP_B_STAT':None,'ADC_SPARE1':'ADC_NTCI','ADC_SPARE2':'ADC_NTCE',
      'I2C_HP_SCL':'I2C_HP_SCL','I2C_HP_SDA':'I2C_HP_SDA','I2C_AFE_SCL':'I2C_AFE_SCL','I2C_AFE_SDA':'I2C_AFE_SDA','I2C_MUX_SCL':'I2C_MUX_SCL','I2C_MUX_SDA':'I2C_MUX_SDA',
      'AFE_ALERT':'AFE_ALERT','AFE_RST_SHUT':'AFE_RST_SHUT','AFE_DFETOFF_SUP':'AFE_DFETOFF_SUP','SUP_OPEN_A_N':'SUP_OPEN_A','SUP_OPEN_B_N':'SUP_OPEN_B','BOARD_PRESENT':'BANK_B_CLOSE','TCA_RST_N':'TCA_RST_N'}
    for k in range(1,7): REN[f'IMON{k}']=f'ADC_IM{k}'; REN[f'PORT_EN{k}']=f'PORT_REQ{k}'
    RE=['EN_24V','EN_12V_JETSON','EN_12V_THIRD_M','EN_12V_AUX_M','EN_12V_PI','EN_7V4','EN_5V','EN_3V3','EN_SPARE_M','EN_CLEAN5']
    for k,v in enumerate(RE): REN[f'RAIL_EN{k+1}']=v
    nets={str(i):None for i in range(1,101)}
    for p in d['pin_plan']:
        nets[str(p['pin'])]=REN.get(p['signal'],p['signal'])
    nets.update({'11':'+3V3_AON','12':'+3V3_A','13':'+3V3_AON','14':'GND','63':'GND','64':'+3V3_AON','23':'BUCK12_OFF','24':'BUCK24_OFF','15':'PG_12V','16':'PG_24V','18':'RAIL_FLT','19':'RAIL_ALERT_A','20':'RAIL_ALERT_B','1':'EN_CLEAN3','77':'WD_KICK','48':'FLT_A','94':'ESTOP_ARMED','95':'WD_N_M'})
    nic(s,'MCU_ST_STM32G0:STM32G0B1VETx','U1401','STM32G0B1VET6',(101.6,127),'Package_QFP:LQFP-100_14x14mm_P0.5mm',nets,ds='https://www.st.com/resource/en/datasheet/stm32g0b1ve.pdf',L=7.62)
    g=Grid(s,190.5,33.02,13.97,40.64,560)
    g.C('100nF','+3V3_AON','GND',k=4); g.C('4.7uF','+3V3_AON','GND',C603,k=2); g.FB('BLM15','+3V3_AON','+3V3_A'); g.C('100nF','+3V3_A','GND'); g.C('1uF','+3V3_A','GND',C603); g.F('+3V3_A')
    g.FB('BLM15','+3V3_AON','+3V3_HP'); g.C('100nF','+3V3_HP','GND'); g.C('4.7uF','+3V3_HP','GND',C603); g.F('+3V3_HP'); g.C('100nF','SUP_NRST','GND')
    g.nl()
    for k in range(1,7): g.R('1k',f'IMON{k}',f'ADC_IM{k}'); g.C('10nF',f'ADC_IM{k}','GND')
    g.R('1k','MID_SENSE','ADC_MID'); g.C('10nF','ADC_MID','GND'); g.R('1k','VMOT_SENSE','ADC_VMOT'); g.C('10nF','ADC_VMOT','GND')
    g.nl()
    g.R('1M 0.1%','VSYS','VSYS_DIV',R1206); g.R('56k 0.1%','VSYS_DIV','GND'); g.C('1nF','VSYS_DIV','GND'); g.R('1k','VSYS_DIV','ADC_VSYS'); g.C('10nF','ADC_VSYS','GND')
    for ext,adc in (('NTC_BOARD1','ADC_NTC1'),('NTC_B2','ADC_NTC2'),('NTC_B3','ADC_NTC3'),('NTC_BRAKE','ADC_NTCB'),('NTC_INTAKE','ADC_NTCI'),('NTC_EXHAUST','ADC_NTCE')):
        g.R('10k','+3V3_A',ext); g.R('1k',ext,adc); g.C('10nF',adc,'GND')
    g.RT('10k B3435','NTC_B2','GND',R603); g.RT('10k B3435','NTC_B3','GND',R603)
    g.nl()
    for a in ('I2C_HP_SDA','I2C_HP_SCL','I2C_AFE_SDA','I2C_AFE_SCL','I2C_MUX_SDA','I2C_MUX_SCL','I2C_A_SDA','I2C_A_SCL','I2C_B_SDA','I2C_B_SCL','QWIIC_SDA','QWIIC_SCL'): g.R('2.2k',a,'+3V3_AON')
    g.nl()
    g.R('10k','AFE_ALERT','+3V3_AON'); g.R('10k','TCA_RST_N','+3V3_AON'); g.R('100k','EN_CLEAN5','GND'); g.R('100k','EN_CLEAN3','GND'); g.R('10k','PG_12V','+3V3_AON'); g.R('10k','PG_24V','+3V3_AON')
    g.C('100nF','+3V3_AON','GND'); g.R('1k','EN_12V_THIRD_M','EN_12V_THIRD'); g.R('1k','EN_12V_AUX_M','EN_12V_AUX'); g.R('1k','EN_SPARE_M','EN_SPARE')
    s.refc['U']=1401; t=s.nref('U')
    nic(s,'Interface_Expansion:TCA9546APW',t,'TCA9546APWR',(431.8,330.2),'Package_SO:TSSOP-16_4.4x5mm_P0.65mm',{'1':'GND','2':'GND','3':'TCA_RST_N','4':'I2C_A_SDA','5':'I2C_A_SCL','6':'I2C_B_SDA','7':'I2C_B_SCL','8':'GND','9':'QWIIC_SDA','10':'QWIIC_SCL','11':None,'12':None,'13':'GND','14':'I2C_MUX_SCL','15':'I2C_MUX_SDA','16':'+3V3_AON'},ds='https://www.ti.com/lit/ds/symlink/tca9546a.pdf')
    r=s.nref('J'); nic(s,'Connector_Generic:Conn_02x03_Odd_Even',r,'SWD Tag-Connect',(330.2,330.2),'Connector:Tag-Connect_TC2030-IDC-NL_2x03_P1.27mm_Vertical',{'1':'+3V3_AON','2':'SUP_SWDIO','3':'SUP_NRST','4':'SUP_SWCLK','5':'GND','6':None})
    s.text('G0B1VET6 pin plan: parts check 68 of 68 against ST pin data. Extra lines beyond the plan: PC2/PC3 buck off, PF0/PF1 PG, PF3-5 rail flags, PB9 EN_CLEAN3. PF10 was BOARD_PRESENT, now BANK_B_CLOSE. PC9 WD_KICK, PE15 FLT_A, PE0 ESTOP_ARMED, PE1 WD_N read-back, input only, 10k in series. PD8-11, PD15, PD2 are now PORT_REQ, gated in hardware to PORT_EN.\nI2C: 3 hardware buses. I2C1 HP meters, I2C2 AFE, I2C3 to TCA9546A at 0x70 (ch0 meters A, ch1 meters B, ch2 Qwiic, ch3 spare).\nFive NTC channels sit on 10k to the ADC reference. NTC_BOARD1 also reads open when the HP board is missing: replaces BOARD_PRESENT.',(190.5,254),size=1.5)
    s.text('''12 V LOAD SHED (budget sums to 22 A on a 12.5 A converter, DC limit 14.2 A min). U1321 meters the real total on +12V: amps are not summed across rails. Start values, tune on the bench.
1. Above 10.5 A, 500 ms: fans to reduced duty (FANx_PWM).
2. Above 11.0 A, 500 ms: third board off (EN_12V_THIRD).
3. HARDWARE, above 12.0 A, about 150 us: U1321 ALERT latches SHED_N: third board, aux and spare off through 1k series FETs, no firmware. Firmware clears after the total stays under 10.5 A.
4. Above 12.4 A for 1 s with step 3 done: Pi stub and Jetson last, after telling the host. PG_12V falling alone never sheds compute: log only.
PORT SOFTWARE TRIP (firmware): each ADC_IMx, 1-15 A per port, default 15 A on arm and waist, 10 A on aux. Over the limit drops PORT_REQx, then reset needs the 148 ms latch sequence.
WATCHDOG: kick WD_KICK from the main loop only, after the port and pack checks ran. 1.36-1.84 s, not firmware-startable or stoppable.''',(190.5,270),size=1.5)
    return s
# ---------- bridges and hub ----------
def bridges():
    s=base('can_bridges_usb_hub','can_bridges_usb_hub: two STM32G474 CAN bridges, USB2514B hub',1500,'6 CAN buses over one USB-C to the Pi. Hub comes up without the supervisor, so the Pi keeps CAN if it dies.')
    s.paper='A2'; s.refc['U']=1503; s.refc['J']=1502
    def br(x,y,ref,t,a,b,c):
        n={str(i):None for i in range(1,49)}
        n.update({'45':f'CAN{a}_RXD','46':f'CAN{a}_TXD','26':f'CAN{b}_RXD','27':f'CAN{b}_TXD','30':f'CAN{c}_RXD','39':f'CAN{c}_TXD','33':f'USB_{t}_DM','34':f'USB_{t}_DP','37':f'SWDIO_{t}','38':f'SWCLK_{t}','5':f'HSE_I_{t}','6':f'HSE_O_{t}','7':f'NRST_{t}',
          '1':'+3V3_AON','19':'GND','20':'+3V3_AON','21':'+3V3_AON','23':'GND','24':'+3V3_AON','35':'GND','36':'+3V3_AON','47':'GND','48':'+3V3_AON'})
        nic(s,'MCU_ST_STM32G4:STM32G474CETx',ref,'STM32G474CET6',(x,y),'Package_QFP:LQFP-48_7x7mm_P0.5mm',n,ds='https://www.st.com/resource/en/datasheet/stm32g474ce.pdf',L=7.62)
        q=s.nref('Y'); nic(s,'Device:Crystal_GND24',q,'ABM8G-24.000MHZ-4Y-T',(x+76.2,y-30.48),'Crystal:Crystal_SMD_3225-4Pin_3.2x2.5mm',{'1':f'HSE_I_{t}','2':'GND','3':f'HSE_O_{t}','4':'GND'})
        g=Grid(s,x+58.42,y-15.24,13.97,30.48,x+150)
        g.C('12pF C0G',f'HSE_I_{t}','GND'); g.C('12pF C0G',f'HSE_O_{t}','GND'); g.C('100nF',f'NRST_{t}','GND'); g.nl()
        g.C('100nF','+3V3_AON','GND',k=3); g.C('4.7uF','+3V3_AON','GND',C603); g.C('10nF','+3V3_AON','GND'); g.C('1uF','+3V3_AON','GND',C603)
        r=s.nref('J'); nic(s,'Connector_Generic:Conn_02x03_Odd_Even',r,'SWD Tag-Connect',(x+127,y-30.48),'Connector:Tag-Connect_TC2030-IDC-NL_2x03_P1.27mm_Vertical',{'1':'+3V3_AON','2':f'SWDIO_{t}','3':f'NRST_{t}','4':f'SWCLK_{t}','5':'GND','6':None})
    br(76.2,76.2,'U1501','A',1,2,3); br(76.2,203.2,'U1502','B',4,5,6)
    n={'1':'USB_SUP_DM','2':'USB_SUP_DP','3':'USB_A_DM','4':'USB_A_DP','5':'+3V3_AON','6':'USB_B_DM','7':'USB_B_DP','8':'USB_SVC_DM','9':'USB_SVC_DP','10':'+3V3_AON','11':'GND','12':None,'13':None,'14':'CRFILT','15':'+3V3_AON','16':None,'17':None,'18':None,'19':None,'20':'SVC_PWR_EN','21':None,'22':'STRAP_NR1','23':'+3V3_AON','24':'STRAP_CF0','25':'STRAP_CF1','26':'HUB_RST_N','27':'VBUS_DET','28':'STRAP_NR0','29':'+3V3_AON','30':'USB_UP_DM','31':'USB_UP_DP','32':'HUB_XO','33':'HUB_XI','34':'PLLFILT','35':'RBIAS','36':'+3V3_AON','37':'GND'}
    nic(s,'Interface_USB:USB2514B_Bi','U1503','USB2514B-I/M2',(355.6,88.9),'Package_DFN_QFN:QFN-36-1EP_6x6mm_P0.5mm_EP3.7x3.7mm',n,ds='https://ww1.microchip.com/downloads/en/DeviceDoc/00001998A.pdf',L=7.62)
    q=s.nref('Y'); nic(s,'Device:Crystal_GND24',q,'ABM8G-24.000MHZ-4Y-T',(444.5,45.72),'Crystal:Crystal_SMD_3225-4Pin_3.2x2.5mm',{'1':'HUB_XI','2':'GND','3':'HUB_XO','4':'GND'})
    g=Grid(s,431.8,63.5,13.97,30.48,570)
    g.C('12pF C0G','HUB_XI','GND'); g.C('12pF C0G','HUB_XO','GND'); g.R('12.0k 1%','RBIAS','GND'); g.C('100nF','CRFILT','GND'); g.C('100nF','PLLFILT','GND'); g.R('100k','STRAP_NR1','+3V3_AON'); g.R('100k','STRAP_NR0','+3V3_AON'); g.R('10k','STRAP_CF0','GND'); g.R('10k','STRAP_CF1','GND')
    g.nl(); g.R('10k','HUB_RST_N','+3V3_AON'); g.C('100nF','HUB_RST_N','GND'); g.R('100k','UP_VBUS','VBUS_DET'); g.R('100k','VBUS_DET','GND'); g.C('100nF','+3V3_AON','GND',k=6); g.C('4.7uF','+3V3_AON','GND',C603)
    y=256.54
    usb='Connector:USB_C_Receptacle_USB2.0_16P'; ufp='Connector_USB:USB_C_Receptacle_GCT_USB4105-xx-A_16P_TopMnt_Horizontal'
    def uc(x,ref,val,vb,dm,dp,cc1,cc2):
        nic(s,usb,ref,val,(x,y+22.86),ufp,{'A1':'GND','A12':'GND','B1':'GND','B12':'GND','A4':vb,'A9':vb,'B4':vb,'B9':vb,'A5':cc1,'B5':cc2,'A6':dp,'B6':dp,'A7':dm,'B7':dm,'A8':None,'B8':None,'SH':'GND'})
    uc(330.2,'J1501','USB4105-GF-A, up to Pi','UP_VBUS','USB_UP_DM','USB_UP_DP','UP_CC1','UP_CC2')
    uc(482.6,'J1502','USB4105-GF-A, service','SVC_VBUS','USB_SVC_DM','USB_SVC_DP','SVC_CC1','SVC_CC2')
    e='Power_Protection:USBLC6-2SC6'; ef='Package_TO_SOT_SMD:SOT-23-6'
    for x,dm,dp,vb in ((381,'USB_UP_DM','USB_UP_DP','UP_VBUS'),(533.4,'USB_SVC_DM','USB_SVC_DP','SVC_VBUS')):
        r=s.nref('U'); nic(s,e,r,'USBLC6-2SC6',(x,y+15.24),ef,{'1':dm,'2':'GND','3':dp,'4':dp,'5':vb,'6':dm})
    g=Grid(s,330.2,y+66.04,13.97,30.48,570)
    g.R('5.1k','UP_CC1','GND'); g.R('5.1k','UP_CC2','GND'); g.R('56k','RAW_5V','SVC_CC1'); g.R('56k','RAW_5V','SVC_CC2')
    g.C('1uF 10V','RAW_5V','GND',C603); g.C('10uF 10V','SVC_SW','GND',C603); g.C('1nF','SVC_CT','GND'); g.R('100k','SVC_PWR_EN','GND'); g.PF('0.5A polyfuse','SVC_SW','SVC_VBUS','Fuse:Fuse_1206_3216Metric'); g.C('100nF','UP_VBUS','GND')
    lid=custom('TPS22965','tps22965',ren={'9':'EP'})
    nic(s,lid,'U1506','TPS22965DSGR',(177.8,y+45.72),'Package_SON:Texas_DSG0008A_WSON-8-1EP_2x2mm_P0.5mm_EP0.9x1.6mm',{'1':'RAW_5V','2':'RAW_5V','3':'SVC_PWR_EN','4':'RAW_5V','5':'GND','6':'SVC_CT','7':'SVC_SW','8':'SVC_SW','9':'GND'},ds='https://www.ti.com/lit/ds/symlink/tps22965.pdf')
    s.text('Hub: DN1 supervisor, DN2 bridge A, DN3 bridge B, DN4 service. No EEPROM: CFG_SEL 00, NON_REM 11, self powered, individual switching. TEST to GND. OCS_N1-4 open (internal pull-up).\nFDCAN1_RX on PB8-BOOT0: program option bytes to ignore the pin before first run. PA15 released from JTDI for FDCAN3_TX. Firmware: budgetcan_fw, candleLight has no G474 build.\nService USB-C sits on a hub downstream port, so it is a host port: a laptop cannot reach the board through it. Rp 56k, default USB current.',(190.5,20.32),size=1.5)
    return s
# ---------- isolated CAN ----------
ISO_PINS=['TXD','RXD','CANH','CANL']
def isocan():
    s=base('can_phy','can_phy: one CAN transceiver (instanced x6)',1600,'TCAN1462V-Q1, non-isolated, +/-58 V bus pins. Motors and frame already tie CAN to power GND.')
    s.multi=50; s.hiers={k:'passive' for k in ISO_PINS}
    if 'TCAN1462V-Q1' not in G()['CUSTOM']:
        G()['mkic']('TCAN1462V-Q1',[('1','TXD','input'),('2','GND','power_in'),('3','VCC','power_in'),('4','RXD','output'),('5','VIO','power_in'),('6','CANL','bidirectional'),('7','CANH','bidirectional'),('8','STB','input')],'Package_SO:SOIC-8_3.9x4.9mm_P1.27mm','https://www.ti.com/lit/ds/symlink/tcan1462-q1.pdf','TI CAN FD SIC transceiver, +/-58 V bus fault, VIO 1.7-5.5 V')
    nic(s,'marengo:TCAN1462V-Q1','U1601','TCAN1462VDRQ1',(88.9,76.2),'Package_SO:SOIC-8_3.9x4.9mm_P1.27mm',{'1':'TXD','2':'GND','3':'RAW_5V','4':'RXD','5':'+3V3_AON','6':'CANL','7':'CANH','8':'GND'},ds='https://www.ti.com/lit/ds/symlink/tcan1462-q1.pdf')
    g=Grid(s,165.1,33.02,15.24,30.48,390)
    g.C('100nF','RAW_5V','GND'); g.C('4.7uF','RAW_5V','GND',C603); g.C('100nF','+3V3_AON','GND')
    s.text('VCC 5 V, 70 mA dominant max, 130 mA with a bus fault: 0.42 A from RAW_5V for six in dominant. VIO 3.3 V logic. STB to GND: normal mode, it has an internal pull-up.\nBus pins +/-58 V to GND, +/-45 V differential, TXD dominant timeout. Termination and TVS sit at the port connector on HP. Single fault H or L to 54.6 V: the other line follows through the termination, in range.\nH at 54.6 V and L hard to GND together exceeds +/-45 V: the transceiver may die, nothing burns. Not isolated: see RATIONALE-log.',(25.4,134),size=1.5)
    return s
# ---------- IO, fans, e-stop loop ----------
def io():
    s=base('io_fans_estop_loop','io_fans_estop_loop: E-stop loop, fans, user I/O, output connectors',2000,'E-stop path: loop current to opto to inverter to gate-driver enable. No firmware in that path.')
    s.paper='A2'
    s.text('E-stop loop, 6.9 mA: +12V, 680R, NC contacts, brake thermostat, opto A, opto B, 620R, GND. No +12 V on the loop connectors: a pin-to-pin short lands on GND and opens the loop.',(25.4,22.86),size=1.5,bold=True)
    conn(s,4,'E-stop loop',(40.64,50.8),GH(4),['LOOP_OUT','GND','TETH_A','GND'])
    conn(s,2,'tether, shorting dummy',(101.6,50.8),GH(2),['TETH_A','TETH_B'])
    conn(s,4,'wireless contact, shorting plug',(162.56,50.8),GH(4),['TETH_B','GND','LOOP_TH','GND'])
    conn(s,2,'brake thermostat NC',(40.64,157.48),GH(2),['LOOP_TH','LOOP_RET'])
    conn(s,2,'wireless receiver power',(101.6,157.48),GH(2),['WL_12V','GND'])
    conn(s,2,'E-stop reset, NO button',(162.56,157.48),GH(2),['ESTOP_RST_N','GND'])
    g=Grid(s,215.9,35.56,15.24,30.48,560)
    g.R('680R 1W','+12V','LOOP_OUT','Resistor_SMD:R_2512_6332Metric'); g.R('620R 0.25W','LOOP_N','GND',R1206); g.D('1N4148W','LOOP_RET','LOOP_N'); g.Z('SMAJ15A','LOOP_OUT','GND','Diode_SMD:D_SMA'); g.Z('SMAJ15A','LOOP_RET','GND','Diode_SMD:D_SMA'); g.C('100nF','+3V3_AON','GND',k=4); g.PF('0.5A polyfuse','+12V','WL_12V','Fuse:Fuse_1206_3216Metric')
    opt=custom('ISOM8710','isom8710',fp='',ds='https://www.ti.com/lit/ds/symlink/isom8710.pdf')
    if 'SN74LVC1G74' not in G()['CUSTOM']:
        G()['mkic']('SN74LVC1G74',[('1','CLK','input'),('2','D','input'),('6','~{CLR}','input'),('7','~{PRE}','input'),('8','VCC','power_in'),('3','~{Q}','output'),('5','Q','output'),('4','GND','power_in')],'Package_SO:VSSOP-8_2.3x2mm_P0.5mm','https://www.ti.com/lit/ds/symlink/sn74lvc1g74.pdf','D flip-flop with clear and preset')
        G()['mkic']('SN74LVC244APW',[('1','1~{OE}','input'),('2','1A1','input'),('4','1A2','input'),('6','1A3','input'),('8','1A4','input'),('11','2A1','input'),('13','2A2','input'),('15','2A3','input'),('17','2A4','input'),('19','2~{OE}','input'),('20','VCC','power_in'),('18','1Y1','output'),('16','1Y2','output'),('14','1Y3','output'),('12','1Y4','output'),('9','2Y1','output'),('7','2Y2','output'),('5','2Y3','output'),('3','2Y4','output'),('10','GND','power_in')],'Package_SO:TSSOP-20_4.4x6.5mm_P0.65mm','https://www.ti.com/lit/ds/symlink/sn74lvc244a.pdf','Octal tri-state buffer, Ioff')
        G()['mkic']('TPS3431',[('1','VDD','power_in'),('2','CWD','passive'),('3','EN','input'),('5','SET1','input'),('6','WDI','input'),('7','~{WDO}','open_collector'),('8','ENOUT','open_collector'),('4','GND','power_in'),('9','EP','power_in')],'Package_SON:VSON-8-1EP_3x3mm_P0.65mm_EP1.65x2.4mm','https://www.ti.com/lit/ds/symlink/tps3431.pdf','Standard programmable watchdog timer with enable')
    def ch(y,t,an,cat,en,sup,sense,qn):
        nic(s,opt,s.nref('U'),'ISOM8710',(50.8,y+15.24),'marengo:ISOM8710_DFF',{'1':an,'2':cat,'3':'GND','4':'OPTO'+t,'5':'+3V3_AON'})
        nic(s,'74xGxx:74LVC1G14',s.nref('U'),'SN74LVC1G14DBV',(203.2,y+15.24),'Package_TO_SOT_SMD:SOT-23-5',{'1':None,'2':'INV'+t,'3':'GND','4':'OUT'+t,'5':'+3V3_AON'})
        nic(s,'marengo:SN74LVC1G74',s.nref('U'),'SN74LVC1G74DCUR',(254,y+15.24),'Package_SO:VSSOP-8_2.3x2mm_P0.5mm',{'1':'ESTOP_RST','2':'OUT'+t,'3':qn,'4':'GND','5':'ARM'+t,'6':'CLR'+t,'7':'+3V3_AON','8':'+3V3_AON'},ds='https://www.ti.com/lit/ds/symlink/sn74lvc1g74.pdf')
        q=sot(s,'Transistor_FET:Q_NMOS_GSD','2N7002',(330.2,y+15.24),sup,en,'GND')
        g=Grid(s,368.3,y+2.54,15.24,30.48,570)
        g.R('10k Cat 0 filter','OPTO'+t,'INV'+t); g.R('1k','ARM'+t,en); g.R('100k',sup,'+3V3_AON'); g.R('1k','OPTO'+t,sense)
        g.C('100nF','INV'+t,'GND'); g.C('100nF','+3V3_AON','GND')
        nic(s,'74xGxx:74LVC1G08','U2033' if t=='_A' else 'U2034','SN74LVC1G08DBV',(292.1,y+33.02),'Package_TO_SOT_SMD:SOT-23-5',{'1':'OUT'+t,'2':'WD_N','3':'GND','4':'CLR'+t,'5':'+3V3_AON'})
        r=g.R('160k','OPTO'+t,'INV'+t); dnp(s,r)
        r=g.C_dnp('2.2uF','INV'+t,'GND'); r2=g.R('1M','INV'+t,'+3V3_AON'); dnp(s,r2); r3=g.D('BAT54','OPTO'+t,'INV'+t); dnp(s,r3)
    Grid.C_dnp=lambda g,v,a,b:(lambda r:(dnp(g.s,r),r)[1])(g.two('Device:C','C',v,a,b,C603))
    ch(68.58,'_A','LOOP_RET','LOOP_M','ESTOP_EN_A','SUP_OPEN_A','LOOP_SENSE_A','PORT_OE_N')
    ch(116.84,'_B','LOOP_M','LOOP_N','ESTOP_EN_B','SUP_OPEN_B','LOOP_SENSE_B',None)
    s.text('SUP_OPEN high: 2N7002 sinks EN. MCU dead or Hi-Z: pull-up opens the banks. The FET only sinks, so firmware cannot close an open loop.\nCat 1 option: remove the 10k filter, fit the four DNP parts, about 302 ms with the 100 nF left in (242-372 ms). DNP in rev A: Cat 0.\nRe-close latch: loop open, watchdog or power-up clears the flops. Re-arming needs a rising edge from the NO reset button with the loop closed. A held button does nothing. No firmware path.\n10k + 100 nF ahead of the Schmitt input rides through contact chatter, 0.6 to 0.9 ms added to a stop. \nCLR is OUT AND WD_N through a gate, a clean edge: TPS3431 clears the latch at power-up and on timeout, and no capacitor sits on WD_N.\nOne series loop still feeds both channels: a short across one contact pair defeats both. Not Cat 3.',(320.04,152.4),size=1.5)
    nic(s,'74xGxx:74LVC1G14','U2030','SN74LVC1G14DBV',(203.2,173.99),'Package_TO_SOT_SMD:SOT-23-5',{'1':None,'2':'ESTOP_RST_N','3':'GND','4':'ESTOP_RST','5':'+3V3_AON'})
    g=Grid(s,228.6,160.02,15.24,30.48,285); g.R('10k','ESTOP_RST_N','+3V3_AON'); g.C('100nF','ESTOP_RST_N','GND'); g.R('10k','WD_N','+3V3_AON'); g.R('10k','ARM_A','ESTOP_ARMED'); g.R('10k','WD_N','WD_N_M')
    y=185.42
    s.text('Fans, 12 V, 25 kHz PWM open drain (polarity inverts in firmware).',(25.4,y-5.08),size=1.5,bold=True)
    for k in range(1,4):
        x=25.4+(k-1)*88.9
        conn(s,4,f'fan {k}',(x+15.24,y+20.32),'Connector:FanPinHeader_1x04_P2.54mm_Vertical',['GND','+12V',f'FAN{k}_TACH',f'FAN{k}_DRV'])
        sot(s,'Transistor_FET:Q_NMOS_GSD','2N7002',(x+50.8,y+20.32),f'FAN{k}_PWM',f'FAN{k}_DRV','GND')
    g=Grid(s,292.1,y+2.54,15.24,30.48,570)
    for k in range(1,4): g.R('10k',f'FAN{k}_TACH','+3V3_AON')
    for k in range(1,4): g.R('100k',f'FAN{k}_PWM','GND')
    s.text('Hardware watchdog and port gate. Starts itself at power-up, no firmware can start or stop it:\nSET1 high, CWD open = 1.6 s (1.36-1.84 s). Timeout pulls WD_N low, clears both E-stop latches.\nPORT_OE_N follows latch A, so every PORT_EN tri-states and its 100k pulls it off. Compute rails untouched.',(419.1,185),size=1.5)
    nic(s,'marengo:TPS3431','U2031','TPS3431SDRBR',(530.86,226.06),'Package_SON:VSON-8-1EP_3x3mm_P0.65mm_EP1.65x2.4mm',{'1':'+3V3_AON','2':None,'3':None,'4':'GND','5':'+3V3_AON','6':'WD_KICK','7':'WD_N','8':'WD_N','9':'GND'},ds='https://www.ti.com/lit/ds/symlink/tps3431.pdf')
    nic(s,'marengo:SN74LVC244APW','U2032','SN74LVC244APWR',(530.86,289.56),'Package_SO:TSSOP-20_4.4x6.5mm_P0.65mm',{'1':'PORT_OE_N','2':'PORT_REQ1','4':'PORT_REQ2','6':'PORT_REQ3','8':'PORT_REQ4','11':'PORT_REQ5','13':'PORT_REQ6','15':'GND','17':'GND','19':'PORT_OE_N','20':'+3V3_AON','18':'PORT_EN1','16':'PORT_EN2','14':'PORT_EN3','12':'PORT_EN4','9':'PORT_EN5','7':'PORT_EN6','5':None,'3':None,'10':'GND'},ds='https://www.ti.com/lit/ds/symlink/sn74lvc244a.pdf')
    g=Grid(s,441.96,210.82,15.24,30.48,500); g.C('100nF','+3V3_AON','GND'); g.C('100nF','+3V3_AON','GND'); g.R('100k','WD_KICK','GND')
    g=Grid(s,518.16,68.58,15.24,30.48,555)
    for k in range(1,7): g.R('100k',f'PORT_REQ{k}','GND')
    y=236.22
    s.text('User I/O: 6-pin shell cable, the plan\'s 4 pins cannot carry RGB + button. Bench jumper, buzzer, charge LED, Qwiic, air NTC.',(25.4,y-5.08),size=1.5,bold=True)
    conn(s,6,'shell: button + RGB',(40.64,y+20.32),GH(6),['+3V3_AON','LR','LG','LB','POWER_BTN','GND'])
    conn(s,2,'charge LED',(101.6,y+15.24),GH(2),['LC','GND'])
    conn(s,2,'bench jumper',(152.4,y+15.24),HDR2,['BENCH_JUMPER','GND'])
    conn(s,4,'Qwiic',(203.2,y+15.24),'Connector_JST:JST_SH_SM04B-SRSS-TB_1x04-1MP_P1.00mm_Horizontal',['GND','+3V3_AON','QWIIC_SDA','QWIIC_SCL'])
    conn(s,2,'intake NTC',(254,y+15.24),GH(2),['NTC_INTAKE','GND']); conn(s,2,'exhaust NTC',(292.1,y+15.24),GH(2),['NTC_EXHAUST','GND'])
    g=Grid(s,342.9,y+2.54,15.24,30.48,570)
    g.R('330R','LED_R','LR'); g.R('330R','LED_G','LG'); g.R('330R','LED_B','LB'); g.R('330R','CHARGE_LED','LC'); g.R('10k','POWER_BTN','+3V3_AON'); g.R('10k','BENCH_JUMPER','+3V3_AON'); g.D('BAT54','POWER_BTN','TS2_WAKE'); g.R('100k','BUZZER','GND')
    g.nl(); g.D('1N4148W','RAW_5V','BUZ_D'); g.TP('LOOP_OUT'); g.TP('LOOP_RET')
    sot(s,'Transistor_FET:Q_NMOS_GSD','2N7002',(431.8,y+45.72),'BUZZER','BUZ_D','GND')
    nic(s,'Device:Buzzer',s.nref('LS'),'PS1240P02BT',(482.6,y+45.72),'Buzzer_Beeper:Buzzer_TDK_PS1240P02BT_D12.2mm_H6.5mm',{'1':'RAW_5V','2':'BUZ_D'})
    y=320.04
    s.text('Rail outputs, one Micro-Fit each. TS2 wake: button low pulls AFE TS2 to VSS through 10k + BAT54 with the board off.',(25.4,y-5.08),size=1.5,bold=True)
    for k,net in enumerate(['OUT_24V','OUT_12V_JETSON','OUT_12V_THIRD','OUT_12V_AUX','OUT_12V_PI_STUB','OUT_7V4_SERVO','OUT_5V_PERIPH','OUT_3V3_PERIPH','OUT_SPARE','+5V_CLEAN','+3V3_CLEAN']):
        GLOBALS.add(net); conn(s,2,net.lstrip('+'),(25.4+k*50.8,y+15.24),MF2,[net,'GND'])
    return s
# ---------- stub boards ----------
def pistub():
    s=base('pi_power_stub','pi_power_stub: 12 V to 5.1 V 6 A at the Pi',2100,'Own 2-layer board. Local nets; harness map checked in check.py.')
    lid=custom('TPS56637','tps56637_7v4'); t='_S'; v='VIN_12V'
    nic(s,lid,'U2101','TPS56637RPAR',(106.68,63.5),'marengo:TPS56637_RPA_VQFN-HR-10',{'1':'ENB'+t,'2':'FB'+t,'3':'GND','4':None,'5':None,'6':'SW'+t,'7':'BOOT'+t,'8':v,'9':'GND','10':None},ds='https://www.ti.com/lit/ds/symlink/tps56637.pdf')
    conn(s,2,'12 V in',(40.64,63.5),MF2,[v,'GND']); conn(s,2,'5.1 V out, 8.5 A per contact',(165.1,63.5),MF2,['V5V1','GND'])
    g=Grid(s,203.2,38.1,13.97,30.48,395)
    g.L('XAL6060-562MEC 5.6uH','SW'+t,'V5V1',XAL('XAL6060-XXX')); g.R('75k 1%','V5V1','FB'+t); g.R('10k 1%','FB'+t,'GND'); g.R('20k','V5V1','FF'+t); g.C('220pF C0G','FF'+t,'FB'+t)
    g.C('10uF 50V',v,'GND',C1206,k=2); g.C('100nF',v,'GND')
    g.nl(); g.C('100nF','BOOT'+t,'SW'+t); g.C('22uF 25V','V5V1','GND',C1210,k=2); g.R('169k',v,'ENB'+t); g.R('36.1k','ENB'+t,'GND'); g.F(v)
    s.text('Output pigtail goes to a USB-C plug. A 2x3 housing on the GPIO header would put all 6 A through one GND pin (pin 6) and cannot lock.\nSet usb_max_current_enable=1 on the Pi. PG left open: no logic rail on the stub.',(25.4,109.22),size=1.5)
    return s
def packsense():
    s=base('pack_sense_board','pack_sense_board: tap fuses, NTC, presence loop',2200,'Own 2-layer board at the pack. Pin map must match HP J2. Local nets; harness map checked in check.py.')
    s.refc['J']=2201
    PIN={1:'NTC1',11:'NTC2',2:'NTC3',12:None,3:'NTC_RET',13:'PRES',4:'B0_F',14:'B1_F',5:'B2_F',15:'B3_F',6:'B4_F',16:'B5_F',7:'B6_F',17:'B7_F',8:'B8_F',18:'B9_F',9:'B10_F',19:'B11_F',10:'B12_F',20:'B13_F'}
    nic(s,'Connector_Generic:Conn_02x10_Top_Bottom','J2201','Micro-Fit 2x10',(63.5,63.5),'Connector_Molex:Molex_Micro-Fit_3.0_43045-2000_2x10_P3.00mm_Horizontal',{str(k):v for k,v in PIN.items()})
    conn(s,16,'cell taps B0..B13, presence loop',(355.6,76.2),'Connector_PinSocket_2.54mm:PinSocket_1x16_P2.54mm_Vertical',['PRES','NTC_RET']+[f'B{i}' for i in range(14)])
    g=Grid(s,114.3,38.1,29.21,30.48,295)
    for i in range(14): g.Fu('1A 3-148-693 USE 2410',f'B{i}',f'B{i}_F','marengo:Fuse_2410')
    g.nl(101.6)
    for k in (1,2,3): g.RT('10k 103AT, on leads',f'NTC{k}','NTC_RET','marengo:Wire_Pads_2_NTC')
    s.text('Fuse at the cell, one per tap. Schurter USE 2410 1 A: 79 mOhm, 200 A at 86 VDC, tau under 0.05 ms, against 146 A prospective. Balance adds 24 mV of sense error at 150 mA across two taps.\nFootprint is the Schurter land: 2.5 x 2.8 mm pads, 3.0 mm gap (data sheet p.2).\nPresence: pin 13 goes out through the cell tap plug (J2202 pins 1, 2 looped in the harness, beside B0, away from B13) and back on pin 3, so either plug out opens DSG and CHG. B0 and B13 are their own Kelvin tabs: no pack current in a sense wire. NTC_RET returns on pin 3 to VSS, no cell connection.',(25.4,152.4),size=1.5)
    return s
# ---------- board tops ----------
CONN_A=['VSYS']*6+['GND']*3+['ESTOP_EN_A','ESTOP_EN_B','GND','TS2_WAKE','BANK_B_CLOSE']+[f'PORT_EN{k}' for k in range(1,7)]+['I2C_AFE_SDA','I2C_AFE_SCL','AFE_ALERT','AFE_DFETOFF_SUP','AFE_RST_SHUT','I2C_HP_SDA','I2C_HP_SCL','+3V3_HP','CLAMP_STATUS','CHARGER_PRESENT']
CONN_B=sum([[f'CAN{k}_H',f'CAN{k}_L',('FLT_A' if k==1 else 'GND')] for k in range(1,7)],[])+['GND']+[f'IMON{k}' for k in range(1,7)]+['MID_SENSE','VMOT_SENSE','NTC_BOARD1','NTC_BRAKE','GND']
assert len(CONN_A)==30 and len(CONN_B)==30
def b2b(s,x,y,ra,rb):
    lid='Connector_Generic:Conn_02x15_Odd_Even'
    for r,c,xx,nm in ((ra,CONN_A,x,'power and logic'),(rb,CONN_B,x+76.2,'CAN and analog')):
        nic(s,lid,r,'2x15 2 mm box, '+nm,(xx,y),B2BFP,{str(i+1):v for i,v in enumerate(c)})
def hp_extras(HP):
    HP.text('Board-to-board: two 2x15 2 mm ribbons to LP, pin for pin. No stock 2x30 latched header exists. All 60 pins used.',(38.1,236.22),size=1.5,bold=True)
    b2b(HP,63.5,269.24,'J101','J102')
    g=Grid(HP,241.3,246.38,15.24,30.48,540)
    g.R('1M','PACK_TOP','PACK_PRESENT_N',R1206); g.Z('MMSZ4684 3.3V','PACK_PRESENT_N','VSS'); g.R('100k','DFETOFF','VSS'); g.RT('10k B3435','BOARD_NTC','VSS',R603); g.RT('10k B3435','NTC_BOARD1','GND',R603)
    nic(HP,'Diode:BAT54C','D103','BAT54C',(330.2,256.54),SOT23,{'1':'PACK_PRESENT_N','2':'AFE_DFETOFF_SUP','3':'DFETOFF'})
    g=Grid(HP,368.3,246.38,15.24,30.48,540)
    g.R('100R 1W','V_PORT6','FI_V',R2512); g.TP('VSYS')
    nic(HP,'Connector_Generic:Conn_02x04_Odd_Even','J103','CAN fault injection, CAN6',(431.8,256.54),'Connector_PinHeader_2.54mm:PinHeader_2x04_P2.54mm_Vertical',{'1':'CAN6_H','2':'CAN6_L','3':'CAN6_H','4':'GND','5':'CAN6_L','6':'GND','7':'CAN6_H','8':'FI_V'})
    HP.text('DFETOFF, active high, BOTHOFF (config 0x42): pack sense harness or cell plug pulled, or supervisor backup, opens DSG and CHG. E-stop loop deliberately not wired here: it would deadlock boot (12 V comes after DSG) and drop compute.\nPull-up from PACK_TOP, not LP: it holds with the LP rails down. 54 uA through the presence loop when mated.\nFault injection: test mode, disarmed. Short H-L, H-GND, L-GND, H-VBUS via 100R.',(241.3,284.48),size=1.5)
def lp_extras(LP):
    LP.text('Board-to-board: mirror of HP J101, J102.',(38.1,195.58),size=1.5,bold=True)
    b2b(LP,63.5,228.6,'J2301','J2302')

# ---------- balance FETs on the pack sheet (SLUAA81A Fig 2-2)
def balance(s):
    import re
    for pre in ('R','Q','D'):
        v=[int(m.group(1)) for it in s.items if it[0]=='symbol' and isinstance(it[-1],list) and it[-1][0]=='__inst__' for m in [re.fullmatch(pre+r'(\d+)',it[-1][1])] if m]
        s.refc[pre]=max(v+[s.refc.get(pre,0)])
    s.text('External balance, SLUAA81A Fig 2-2: internal FET current through VC(n-1) input resistor gives Vgs. 3.9 V: 1.59 V min (RCB 46 R). SSM3K56FS is specified at 1.5 V for 50 mA only and 1.8 V for 200 mA: check Rds at 150 mA on the bench, top cell worst case 2.35 V.\nRbal 27R: 140-150 mA external at 3.9-4.2 V, plus 17-19 mA internal. Cell Balance Max Cells 4: about 2.5 W total.',(25.4,288),size=1.5,bold=True)
    for i in range(1,14):
        k=(i-1)%7; y=320+((i-1)//7)*50; x=38+k*78
        src=f'B{i-1}'
        g=Grid(s,x,y,17.78,30,x+70)
        g.R('27R 1W 2512',f'B{i}',f'BD{i}','Resistor_SMD:R_2512_6332Metric')
        sot(s,'Transistor_FET:Q_NMOS_GSD','SSM3K56FS',(x+24,y),f'BG{i}',f'BD{i}',src,fp='Package_TO_SOT_SMD:SOT-416')
        g.x=x+38; g.R('1k',f'BG{i}',f'VC{i-1}'); g.Z('BZT52C5V1',f'BG{i}',src,'Diode_SMD:D_SOD-123')
