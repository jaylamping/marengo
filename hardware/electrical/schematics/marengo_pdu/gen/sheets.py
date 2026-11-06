"""Sheets after pack_input_shunt_afe. G is gen.py's globals."""
G={}
BMS_PINS=[('PACK_TOP','passive'),('VSYS','passive'),('CHG_GATE','input'),('DSG_GATE','input'),('PDSG','input'),('CHARGER_PRESENT','output'),('I2C_HP_SDA','bidirectional'),('I2C_HP_SCL','input'),('+3V3_HP','passive')]
R603='Resistor_SMD:R_0603_1608Metric';C603='Capacitor_SMD:C_0603_1608Metric';C1206='Capacitor_SMD:C_1206_3216Metric'
SOD='Diode_SMD:D_SOD-123'
def base(key,title,base_ref,comment=None):
    s=G['Sheet'](key,title,'A3',comment=comment); s.file=key+'.kicad_sch'
    s.refc={p:base_ref for p in 'RCDQUJLFY'}; s.refc.update({'TP':base_ref,'RS':base_ref,'#PWR':base_ref,'#FLG':base_ref,'FB':base_ref,'RT':base_ref,'LS':base_ref})
    return s
def stub(s,p,net,d='R',L=2.54):
    dx,dy={'R':(L,0),'L':(-L,0),'U':(0,-L),'D':(0,L)}[d]; e=(p[0]+dx,p[1]+dy)
    s.wire(p,e); s.label(net,e,{'R':0,'L':180,'U':90,'D':270}[d]); return e
def gnd(s,p): s.sym('power:GND',s.nref('#PWR'),'GND',p,0,hideref=True)
def rvert(s,at,val,fp=R603,**k):
    s.sym('Device:R',s.nref('R'),val,at,0,fp,**k); return (at[0],at[1]-3.81),(at[0],at[1]+3.81)
def rhor(s,at,val,fp=R603,**k):
    k.setdefault('refat',(at[0],at[1]-2.54)); k.setdefault('valat',(at[0],at[1]+2.54))
    s.sym('Device:R',s.nref('R'),val,at,90,fp,**k); return (at[0]-3.81,at[1]),(at[0]+3.81,at[1])
def cvert(s,at,val,fp=C603,**k):
    s.sym('Device:C',s.nref('C'),val,at,0,fp,**k); return (at[0],at[1]-3.81),(at[0],at[1]+3.81)
def dvert(s,at,val,lid='Device:D_Zener',fp=SOD,**k):
    """rot 90 of K-left symbol: cathode at top"""
    r=s.nref('D'); s.sym(lid,r,val,at,270,fp,**k); return (at[0],at[1]-3.81),(at[0],at[1]+3.81)
def ic(s,lid,ref,val,at,fp,nets,hier=None,ds='',rot=0,L=5.08,**k):
    """place symbol, drop a labelled stub on every pin. nets: pin->net, None=no-connect, 'GND'=ground symbol."""
    l=G['libsym'](lid); k.setdefault('refat',(at[0],at[1]-G.get('ICH',16.51))); k.setdefault('valat',(at[0],at[1]+G.get('ICH',16.51)))
    s.sym(lid,ref,val,at,rot,fp,ds=ds,**k)
    seen={}
    for u in G['getall'](l,'symbol'):
        for p in G['getall'](u,'pin'):
            num=str(G['get'](p,'number')[1]); a=G['get'](p,'at'); ang=(int(float(a[3]))+rot)%360
            q=G['pos'](l,num,at,rot); d={0:'L',180:'R',90:'D',270:'U'}[ang]
            net=nets[num]
            if q in seen: assert seen[q]==net,(ref,num); continue
            seen[q]=net
            dx,dy={'R':(L,0),'L':(-L,0),'U':(0,-L),'D':(0,L)}[d]; e=(q[0]+dx,q[1]+dy)
            if net is None: s.nc(q); continue
            s.wire(q,e)
            if net=='GND': gnd(s,e) if d=='D' else gsym(s,e,d)
            elif hier and net in hier: s.hier(net,e,{'R':0,'L':180,'U':90,'D':270}[d],hier[net])
            elif net in GLOBALS: s.glabel(net,e,{'R':0,'L':180,'U':90,'D':270}[d])
            else: s.label(net,e,{'R':0,'L':180,'U':90,'D':270}[d])
def gsym(s,p,d):
    s.sym('power:GND',s.nref('#PWR'),'GND',p,{'L':270,'R':90,'U':180}[d],hideref=True)
def bms():
    pos=G['pos'];libsym=G['libsym']
    s=base('bms_fets_precharge','bms_fets_precharge: BMS CHG/DSG banks, predischarge, charge input',200,'Fig 16-1, Fig 16-3 of TI SLUSE13B. High side, common drain, common port.')
    GLOBALS.update({'VSYS_SENS_P','VSYS_SENS_N'})
    FET='Transistor_FET:IPT015N10N5'; ft=libsym(FET); FP='Package_TO_SOT_SMD:Infineon_PG-HSOF-8-1'
    YF=76.2; YT=66.04; YB=86.36
    xc=[60.96+27.94*k for k in range(4)]; xd=[187.96+27.94*k for k in range(4)]
    def bank(xs,prefix):
        tops=[];bots=[]
        for x in xs:
            q=s.nref('Q'); at=(x,YF); s.sym(FET,q,'IPT015N10N5',at,0,FP,refat=(x+5.08,YF-7.62),valat=(x+5.08,YF+7.62))
            d=pos(ft,'3',at,0); so=pos(ft,'2',at,0); g=pos(ft,'1',at,0)
            s.wire(d,(d[0],YT)); s.wire(so,(so[0],YB)); tops.append(d[0]); bots.append(so[0])
            a,b=rhor(s,(g[0]-5.08,g[1]),'4.7R',refat=(g[0]-5.08,g[1]-2.54),valat=(g[0]-5.08,g[1]+2.54)); s.wire(b,g); stub(s,a,prefix,'L',1.27)
        return tops,bots
    t1,b1=bank(xc,'CHG_G'); t2,b2=bank(xd,'DSG_G')
    XSOURCE=t2[-1]+17.78      # PDSG FET source drop
    XV=XSOURCE+50.8           # VSYS column
    # CD bus: all drains, runs on to the PDSG FET
    s.wire((t1[0],YT),(XSOURCE,YT)); s.label('CD',(167.64,YT),90)
    for x in t1+t2[:]: s.junc((x,YT))
    # stack bus
    s.wire((30.48,YB),(b1[-1],YB)); s.hier('PACK_TOP',(30.48,YB),180)
    for x in b1[:-1]: s.junc((x,YB))
    # DSG source bus
    XS=XSOURCE+7.62
    s.wire((b2[0],YB),(XS,YB))
    for x in b2[1:]: s.junc((x,YB))
    s.label('DSG_OUT',(b2[-1]+7.62,YB),90)
    s.text('CHG bank: sources on the stack',(xc[0],YB+3.81),size=1.5,bold=True)
    s.text('DSG bank: sources on the output',(xd[0],YB+3.81),size=1.5,bold=True)
    s.text('Common drain (CD): Fig 16-3 pin numbering, gates referenced to the outer sources. Body diodes oppose each other.',(xc[0],YT-5.08),size=1.5,bold=True)
    # shunt DSG_OUT -> VSYS
    ls=libsym('Device:R_Shunt'); RS=(XS+7.62,YB+0)
    s.sym('Device:R_Shunt','RS201','0.3mR CSS4J-4026 5W',RS,90,'Resistor_SMD:R_Shunt_Isabellenhuette_BVR4026',refat=(RS[0],RS[1]-7.62),valat=(RS[0],RS[1]-5.08))
    p1,p2,p3,p4=[pos(ls,x,RS,90) for x in '1234']
    s.wire((XS,YB),p1); s.junc((XS,YB))
    # PDSG predischarge: S at CD, 100R to DSG_OUT
    PF='Transistor_FET:Q_PMOS_GDS'; lp=libsym(PF); Q=(XSOURCE+7.62,40.64)
    s.sym(PF,'Q209','DMP10H400SE',Q,270,'Package_TO_SOT_SMD:SOT-223-3_TabPin2',refat=(Q[0]+6.35,Q[1]-6.35),valat=(Q[0]+6.35,Q[1]-3.81))
    d=pos(lp,'2',Q,270); so=pos(lp,'3',Q,270); g=pos(lp,'1',Q,270)
    s.wire((XSOURCE,YT),(XSOURCE,so[1])); s.wire((XSOURCE,so[1]),so)
    stub(s,d,'PDSG_R','R',5.08)
    x1=d[0]+12.7; s.label('DSG_OUT',(x1,YB-7.62),90); s.wire((x1,YB-7.62),(XS,YB-7.62)); s.wire((XS,YB-7.62),(XS,YB))
    nic(s,G['pwr263s'](),s.nref('R'),'100R PWR263S-35-1000F',(XV+38.1,50.8),G['BAR'],{'1':'PDSG_R','2':'GND','3':'DSG_OUT'})
    # gate-source 1M
    a,b=rhor(s,(XSOURCE-5.08,so[1]-7.62),'1M',refat=(XSOURCE-5.08,so[1]-10.16),valat=(XSOURCE-5.08,so[1]-5.08))
    # a left, b right
    s.wire((XSOURCE,so[1]),(XSOURCE-10.16,so[1])); s.wire((XSOURCE-10.16,so[1]),(XSOURCE-10.16,a[1])); s.wire((XSOURCE-10.16,a[1]),a); s.junc((XSOURCE,so[1]))
    s.wire(b,g)
    jy=b[1]; s.wire((XSOURCE+2.54,jy),(XSOURCE+2.54,jy-5.08)); s.hier('PDSG',(XSOURCE+2.54,jy-5.08),90,'input'); s.junc((XSOURCE+2.54,jy))
    s.text('PDSG FET bypasses DSG only (Fig 16-1): source at CD. Driver sinks 30 uA, so gate-source >= 1M (p45).',(XSOURCE-40.64,20.32),size=1.27)
    # VSYS column
    y=YB; s.wire(p4,(XV,y)); s.hier('VSYS',(XV+10.16,y),0); s.wire((XV,y),(XV+10.16,y))
    s.junc((XV,y))
    # gate networks: row at Y1 (CHG), Y2 (DSG)
    Y1=109.22
    s.hier('CHG_GATE',(38.1,Y1),180,'input'); s.wire((38.1,Y1),(45.72,Y1))
    a,b=rhor(s,(49.53,Y1),'5.1k')
    n1=(58.42,Y1); s.wire(b,n1); s.junc(n1)
    # CHG gate node: to 10M to stack, zener
    s.wire(n1,(73.66,Y1)); s.junc((73.66,Y1)); s.wire((73.66,Y1),(93.98,Y1)); s.label('CHG_G',(93.98,Y1))
    a,b=rvert(s,(58.42,Y1+7.62),'10M'); s.wire(n1,a); stub(s,b,'PACK_TOP','D')
    a,b=dvert(s,(73.66,Y1+7.62),'MMSZ5246B 16V'); s.wire((73.66,Y1),a); stub(s,b,'PACK_TOP','D')
    s.text('CHG drive, Fig 16-3 R37/R31/D5.',(38.1,Y1-7.62),size=1.27,bold=True)
    # DSG
    Y2=147.32
    s.hier('DSG_GATE',(38.1,Y2),180,'input'); s.wire((38.1,Y2),(43.18,Y2))
    a,b=rhor(s,(46.99,Y2),'5.1k')
    a2,b2_=rhor(s,(58.42,Y2),'5.1k'); s.wire(b,a2)
    N=(68.58,Y2); s.wire(b2_,N); s.junc(N)
    # fast off path: DSG pin -> cathode, anode -> 220R -> gate node
    Y3=Y2+10.16
    s.wire((43.18,Y2),(43.18,Y3)); s.junc((43.18,Y2))
    s.sym('Device:D',s.nref('D'),'BAV19W',(46.99,Y3),0,SOD,refat=(46.99,Y3-2.54),valat=(46.99,Y3+2.54))
    a3,b3=rhor(s,(58.42,Y3),'220R',refat=(58.42,Y3-2.54),valat=(58.42,Y3+2.54)); s.wire((50.8,Y3),a3)
    nic(s,'Transistor_BJT:Q_PNP_BEC','Q211','MMBT2907A',(177.8,175.26),'Package_TO_SOT_SMD:SOT-23',{'1':'DSG_OFF_B','2':'DSG_G','3':'DSG_OFF_C'})
    s.wire(b3,(66.04,Y3)); s.label('DSG_OFF_B',(66.04,Y3))
    nic(s,'Device:R',s.nref('R'),'10k',(149.86,175.26),'Resistor_SMD:R_0402_1005Metric',{'1':'DSG_OFF_B','2':'DSG_G'})
    s.wire(N,(78.74,Y2)); s.junc((78.74,Y2)); s.wire((78.74,Y2),(93.98,Y2)); s.junc((93.98,Y2)); s.wire((93.98,Y2),(114.3,Y2)); s.label('DSG_G',(114.3,Y2))
    a,b=rvert(s,(78.74,Y2+7.62),'10M'); s.wire((78.74,Y2),a); stub(s,b,'DSG_OUT','D')
    a,b=dvert(s,(93.98,Y2+7.62),'MMSZ5246B 16V'); s.wire((93.98,Y2),a); stub(s,b,'DSG_OUT','D')
    s.text('DSG drive. On: 10.2k (Fig 16-3 R39+R41). Off: PNP Q211 local turn-off, base through 220R in series + diode to the DSG pin, 10k base-emitter, 15R in the collector. Not the 7.5k of Fig 16-3 (see note 2).',(38.1,Y2-7.62),size=1.27,bold=True)
    # Q210 negative PACK+ gate clamp
    NF='Transistor_FET:Q_NMOS_GSD'; ln=libsym(NF); Q2=(165.1,Y2)
    s.sym(NF,'Q210','BSS123',Q2,0,'Package_TO_SOT_SMD:SOT-23',refat=(Q2[0]+6.35,Q2[1]-1.27),valat=(Q2[0]+6.35,Q2[1]+1.27))
    d=pos(ln,'3',Q2,0); so=pos(ln,'2',Q2,0); g=pos(ln,'1',Q2,0)
    stub(s,d,'DSG_G','U'); stub(s,so,'DSG_OUT','D')
    s.wire(g,(148.59,g[1])); s.junc((154.94,g[1]))
    a,b=rvert(s,(154.94,g[1]+7.62),'10M'); s.wire((154.94,g[1]),a); stub(s,b,'DSG_OUT','D')
    a,b=dvert(s,(139.7,g[1]+7.62),'MMSZ5246B 16V'); s.wire((148.59,g[1]),(139.7,g[1])); s.wire((139.7,g[1]),a); stub(s,b,'DSG_OUT','D'); s.junc((148.59,g[1]))
    a,b=rvert(s,(124.46,g[1]+7.62),'10k'); s.wire((139.7,g[1]),(124.46,g[1])); s.junc((139.7,g[1])); s.wire((124.46,g[1]),a)
    s.sym('Device:D',s.nref('D'),'BAV19W',(124.46,g[1]+15.24),270,SOD,refat=(128.27,g[1]+15.24),valat=(128.27,g[1]+17.78)); gnd(s,(124.46,g[1]+19.05))
    s.text('Fig 16-3 Q4 D7 R33 R35 D8: if the output is yanked below GND, Q210 holds the DSG gate at its source.',(127,Y2-13.97),size=1.27,bold=True)
    # CD snubber, stack TVS
    Xc=229.87; Y4=109.22
    stub(s,(Xc,Y4),'PACK_TOP','U'); a,b=cvert(s,(Xc,Y4+3.81),'100nF 100V',fp=C1206); a2,b2_=cvert(s,(Xc,Y4+11.43),'100nF 100V',fp=C1206); stub(s,b2_,'DSG_OUT','D')
    s.text('Fig 16-3 C28 C29: across the FETs,\ntwo in series.',(Xc+7.62,Y4-3.81),size=1.27)
    Xt=273.05
    stub(s,(Xt,Y4),'PACK_TOP','U')
    a,b=dvert(s,(Xt,Y4+3.81),'SMDJ58A',fp='Diode_SMD:D_SMC'); gnd(s,b)
    s.text('Stack spike: 4 x 100 uF hybrid (right, below U201), 2 uH, 600 A, 90 mOhm pack:\npeak about 77 V at -20% C. Pack fuse covers a shorted part here. TVS is a backstop (conducts from 64.4 V).',(Xt+5.08,Y4-3.81),size=1.27)
    for _i,_x in enumerate((325.12,345.44,365.76,386.08)):
        s.sym('Device:C_Polarized',s.nref('C'),'100uF 80V HHXF800GRA101MJH0G',(_x,170.18),0,'Capacitor_SMD:CP_Elec_10x14.3',refat=(_x+1.27,166.37),valat=(_x-15.24,179.07+_i*2.54)); stub(s,(_x,166.37),'PACK_TOP','U'); gnd(s,(_x,173.99))
    # VSYS shunt meter
    def _gs(p,net):
        e=(p[0],p[1]+5.08); s.wire(p,e); s.glabel(net,e,270)
    _gs(p2,'VSYS_SENS_P'); _gs(p3,'VSYS_SENS_N')
    U=(XV-30.48,132.08)
    ic(s,'Sensor_Energy:INA228','U201','INA228AIDGSR',U,'Package_SO:TSSOP-10_3x3mm_P0.5mm',{'1':'I2C_HP_SCL','2':'GND','3':None,'4':'I2C_HP_SDA','5':'I2C_HP_SCL','6':'+3V3_HP','7':'GND','8':'VSYS','9':'VSYS_SENS_N','10':'VSYS_SENS_P'},hier={'I2C_HP_SDA':'bidirectional','I2C_HP_SCL':'input','+3V3_HP':'passive','VSYS':'passive'},ds='https://www.ti.com/lit/ds/symlink/ina228.pdf')
    a,b=cvert(s,(U[0]-27.94,U[1]+7.62),'100nF'); s.hier('+3V3_HP',a,180); gnd(s,b)
    s.text('Main path meter, PLAN 3.9. Address 0x4C: A1 to SCL, A0 to GND. +/-40.96 mV range = +/-136.5 A. Bank A short sense reads this shunt too.',(U[0]-27.94,U[1]+20.32),size=1.27)
    # VSYS parts
    xs=[XV,XV+15.24,XV+30.48,XV+45.72]
    s.wire((XV+10.16,YB),(xs[3],YB))
    for x in xs[1:3]: s.junc((x,YB))
    a,b=dvert(s,(xs[0],YB+7.62),'SMDJ58A',fp='Diode_SMD:D_SMC'); s.wire((xs[0],YB),a); gnd(s,b)
    a,b=cvert(s,(xs[1],YB+7.62),'100nF 100V',fp=C1206); s.wire((xs[1],YB),a); a2,b2_=cvert(s,(xs[1],YB+15.24),'100nF 100V',fp=C1206); gnd(s,b2_)
    s.sym('Device:C_Polarized',s.nref('C'),'100uF 80V HHXF800GRA101MJH0G',(xs[2],YB+7.62),0,'Capacitor_SMD:CP_Elec_10x14.3',refat=(xs[2]+1.27,YB+13.97),valat=(xs[2]+1.27,YB+16.51)); s.wire((xs[2],YB),(xs[2],YB+3.81)); gnd(s,(xs[2],YB+11.43))
    s.sym('Connector:TestPoint','TP201','VSYS',(xs[3],YB),0,'TestPoint:TestPoint_Keystone_5015_Micro_Mini',refat=(xs[3]+2.54,YB-5.08),valat=(xs[3]+2.54,YB-2.54))
    # charge input
    Y6=213.36
    l1=libsym('Connector_Generic:Conn_01x01')
    s.sym('Connector_Generic:Conn_01x01','J201','M4 terminal, charger +, 74651194R',(38.1,Y6),180,'TerminalBlock_Wuerth:Wuerth_REDCUBE-THR_WP-THRSH_74651194_THR',refat=(30.48,Y6-2.54),valat=(25.4,Y6+2.54))
    s.sym('Connector_Generic:Conn_01x01','J202','M4 terminal, charger -, 74651194R',(38.1,Y6+25.4),180,'TerminalBlock_Wuerth:Wuerth_REDCUBE-THR_WP-THRSH_74651194_THR',refat=(30.48,Y6+22.86),valat=(25.4,Y6+27.94))
    e=pos(l1,'1',(38.1,Y6),180); e2=pos(l1,'1',(38.1,Y6+25.4),180)
    s.wire(e,(129.54,Y6)); s.label('CHG_IN',(60.96,Y6),90)
    for x in (60.96,76.2,91.44): s.junc((x,Y6))
    s.wire(e2,(60.96,e2[1])); gnd(s,(60.96,e2[1]))
    s.sym('Device:D_TVS',s.nref('D'),'SMDJ58CA',(76.2,Y6+7.62),90,'Diode_SMD:D_SMC',refat=(80.01,Y6+6.35),valat=(80.01,Y6+8.89)); s.wire((76.2,Y6),(76.2,Y6+3.81)); gnd(s,(76.2,Y6+11.43))
    s.sym('Device:D',s.nref('D'),'BAV19W',(91.44,Y6+7.62),90,SOD,refat=(95.25,Y6+6.35),valat=(95.25,Y6+8.89)); s.wire((91.44,Y6),(91.44,Y6+3.81))
    a,b=rvert(s,(91.44,Y6+15.24),'1M'); a2,b2_=rvert(s,(91.44,Y6+22.86),'62k')
    n1=(91.44,Y6+19.05); s.junc(n1); gnd(s,b2_)
    s.wire(n1,(124.46,n1[1])); s.hier('CHARGER_PRESENT',(124.46,n1[1]),0,'output')
    a,b=dvert(s,(104.14,n1[1]+3.81),'MMSZ4685 3.6V'); gnd(s,b); s.junc((104.14,n1[1]))
    a,b=cvert(s,(114.3,n1[1]+3.81),'1nF'); gnd(s,b); s.junc((114.3,n1[1]))
    nic(s,G['v10pm15'](),'D220','V10PM15-M3/I 150V 10A',(149.86,Y6+10.16),'Package_TO_SOT_SMD:TO-277A',{'1':'CHG_IN','2':'CHG_IN','3':'VSYS'})
    s.label('CHG_IN',(129.54,Y6))
    s.text('Charge input, common port: the charger joins on VSYS, DSG body diode side, as the BQ76952 expects\n(PACK pin sees it, p45). Also wakes the AFE from ship mode through LD.\nPLAN 3.7 says reverse-polarity FET. With a reversed charger the blocker sees 54.6 + 54.6 = 109 V: above the\n100 V FET and the LM74700 (75 V, SNOSD17G). So a 150 V Schottky: Vishay V10PM15, pads 1, 2 anode, tab K, 0.6 V, 2.4 W at 4 A. See RATIONALE-log.\nCHARGER_PRESENT: CHG_IN only, so VSYS cannot fake it; high above 34 V, 3.2 V at 54.6 V.',(38.1,Y6-19.05),size=1.27)
    N1="""SIZING (design/README 2.2)
Conduction, 80 A: 2 x 0.6 mOhm hot x 80^2 = 7.7 W total. At 10 to 30 A: 0.1 to 1.1 W for these two banks.
FET: IPT015N10N5 Rds 1.5 mOhm max at Vgs 10 V, Vgs +/-20 V, so 11 V drive and 16 V zeners are in range (p69 needs >= 15 V).
Shunt voltage rule p69: 13S x 5 V = 65 V DC, 130 V peak. 100 V FETs meet DC. Peak: both TVS parts hold it under 100 V."""
    N2="""SOA must cover 600 A prospective for about 50 us. Reworked after gate 3:
1. Prospective current: 600 A / 4 = 150 A per FET. On-state part: 0.6 mOhm x 600 A = 0.36 V, 216 W bank, nothing.
2. Turn-off: TI gives only 2 us typ, 20 us max at 20 nF for the DSG pin, pulsed, stopping at LD (section 16.5). Ours is 48 nF, so a bare 220R is unsupported.
   Q211: base through 220R and D202, about 45 mA, 10k base-emitter. 15R in the collector sets the peak: 0.62 A from 11 V, under the 800 mA rating. All gate charge 844 nC max in about 2 us. 10 us SOA at 54 V is about 300 A per FET: PASSES at 150 A, with sharing unmodelled: scope at bring-up.
3. Spike: p72 warns fast turn-off raises the top of stack. 4 x 100 uF hybrid, pack EMF behind 2 uH and 90 mOhm (PLAN pack R): peak about 77 V at -20% C. FET stays under 100 V without avalanche."""
    N3="""CP1 is 1 uF on sheet 1: 8 x 211 nC max = 1.69 uC at p65's rule, droop 1.7 V from 11 V.
PDSG not PCHG: p45 PDSG charges the LOAD before DSG closes; PCHG charges a LOW BATTERY. README 2.1 described the PDSG job.
100R: peak 54.6^2/100 = 29.8 W. VSYS load 1.46 mF (hold-up via Q1101 body diode, plus VSYS bulk): 2.2 J per power-up, tau 0.146 s. PWR263S chart: 10 J at 100 ms.
Set OTP: Mfg Status Init[FET_EN] = 1 (default 0 is FET test mode: no PDSG, VSYS stays down, the MCU never boots). FET Options[PDSG_EN] = 1, Predischarge Timeout 100 (1.0 s), Stop Delta 2500 mV.\nStop Delta needs under 25 mA, so the timeout must exist: a hard VSYS short ends in SCD.\nEnabled Protections C[SCDL] = 1, SCDL Latch Limit 2, Counter Dec Delay 60 s: a persistent short costs R209 two 30 J pulses, then latches. Recovery: unplug the pack, or firmware SCDL_RECOVER() on an operator command.\nFirmware holds BUCK12_OFF high only while FET Status[PDSG_FET] is set, and releases it on an I2C read failure, so an MCU reset or charger-held VSYS keeps compute up."""
    nic(s,'Device:R',s.nref('R'),'15R',(198.12,175.26),'Resistor_SMD:R_0805_2012Metric',{'1':'DSG_OFF_C','2':'DSG_OUT'})
    s.text(N1,(170.18,195.58),size=1.27); s.text(N2,(182.88,213.36),size=1.27); s.text(N3,(182.88,241.3),size=1.27)
    return s

# ---------- block helpers for the label-style sheets ----------
import json,os
PARTS=os.path.join(os.path.dirname(os.path.abspath(__file__)),'parts')
R402='Resistor_SMD:R_0402_1005Metric';C402='Capacitor_SMD:C_0402_1005Metric';C805='Capacitor_SMD:C_0805_2012Metric';C1210='Capacitor_SMD:C_1210_3225Metric'
GLOBALS=set()
def put(s,p,net,d):
    """end a stub at p with net (ground symbol, global label or local label)"""
    a={'R':0,'L':180,'U':90,'D':270}[d]
    if net=='GND': gnd(s,p) if d=='D' else gsym(s,p,d)
    elif net=='VSS': s.sym('power:VSS',s.nref('#PWR'),'VSS',p,{'D':180,'U':0,'L':90,'R':270}[d],hideref=True)
    elif net in GLOBALS: s.glabel(net,p,a)
    elif getattr(s,'hiers',None) and net in s.hiers: s.hier(net,p,a,s.hiers[net])
    else: s.label(net,p,a)
class Grid:
    def __init__(g,s,x,y,dx=15.24,dy=30.48,xmax=390):
        g.s=s;g.x0=x;g.x=x;g.y=y;g.dx=dx;g.dy=dy;g.xmax=xmax
    def at(g):
        if g.x>g.xmax: g.x=g.x0; g.y+=g.dy
        p=(round(g.x/1.27)*1.27,round(g.y/1.27)*1.27); g.x+=g.dx; return p
    def nl(g,y=None):
        g.x=g.x0; g.y=y if y is not None else g.y+g.dy
    def two(g,lid,pre,val,a,b,fp,rot=0):
        s=g.s; p=g.at(); r=s.nref(pre)
        s.sym(lid,r,val,p,rot,fp,refat=(p[0]+2.54,p[1]-1.27),valat=(p[0]+2.54,p[1]+1.27))
        t=(p[0],p[1]-3.81); bt=(p[0],p[1]+3.81)
        if a: e=(t[0],t[1]-2.54); s.wire(t,e); put(s,e,a,'U')
        else: s.nc(t)
        if b: e=(bt[0],bt[1]+2.54); s.wire(bt,e); put(s,e,b,'D')
        else: s.nc(bt)
        return r
    def F(g,net):
        s=g.s; p=g.at(); s.sym('power:PWR_FLAG',s.nref('#FLG'),'PWR_FLAG',p,180); e=(p[0],p[1]+2.54); s.wire(p,e); put(s,e,net,'D')
    def R(g,val,a,b,fp=R402): return g.two('Device:R','R',val,a,b,fp)
    def C(g,val,a,b,fp=C402,k=1):
        for _ in range(k): g.two('Device:C','C',val,a,b,fp)
    def CP(g,val,a,b,fp): return g.two('Device:C_Polarized','C',val,a,b,fp)
    def L(g,val,a,b,fp): return g.two('Device:L','L',val,a,b,fp)
    def D(g,val,k,an,fp=SOD): return g.two('Device:D','D',val,k,an,fp,rot=270)
    def Z(g,val,k,an,fp=SOD): return g.two('Device:D_Zener','D',val,k,an,fp,rot=270)
def ext(l):
    ys=[]
    for u in G['getall'](l,'symbol'):
        for p in G['getall'](u,'pin'):
            ys.append(float(G['get'](p,'at')[2]))
    return (min(ys),max(ys)) if ys else (-10,10)
def nic(s,lid,ref,val,at,fp,nets,ds='',rot=0,L=5.08):
    at=(round(at[0]/1.27)*1.27,round(at[1]/1.27)*1.27)
    l=G['libsym'](lid); lo,hi=ext(l)
    s.sym(lid,ref,val,at,rot,fp,ds=ds,refat=(at[0]-5.08,at[1]-hi-(11.43)),valat=(at[0]-5.08,at[1]-lo+(11.43)))
    seen={}
    for u in G['getall'](l,'symbol'):
        for p in G['getall'](u,'pin'):
            num=str(G['get'](p,'number')[1]); a=G['get'](p,'at'); ang=(int(float(a[3]))+rot)%360
            q=G['pos'](l,num,at,rot); d={0:'L',180:'R',90:'D',270:'U'}[ang]
            net=nets.get(num,'__miss__'); assert net!='__miss__',(ref,num)
            if q in seen: assert seen[q]==net,(ref,num); continue
            seen[q]=net
            if net is None: s.nc(q); continue
            dx,dy={'R':(L,0),'L':(-L,0),'U':(0,-L),'D':(0,L)}[d]; e=(q[0]+dx,q[1]+dy)
            s.wire(q,e); put(s,e,net,d)
def custom(name,jf,fp=None,ds=None,ren=None,et=None,renum=None):
    d=json.load(open(os.path.join(PARTS,jf+'.json')))
    pins=[]
    for p in d['pins']:
        nm=str(p['name']).split(' ')[0].split('(')[0]; nm=(ren or {}).get(str(p['num']),nm)
        pins.append(((renum or {}).get(str(p['num']),str(p['num'])),nm,(et or {}).get(str(p['num']),p['etype'])))
    names=[p[1] for p in pins]; pins=[(a,b,'passive' if c=='power_out' and names.count(b)>1 else c) for a,b,c in pins]
    if name not in G['CUSTOM']: G['mkic'](name,pins,fp if fp is not None else (d.get('kicad_footprint') or ''),(d.get('datasheet') or {}).get('url',''),d.get('part',''))
    return 'marengo:'+name
MOS5='Package_TO_SOT_SMD:TDSON-8-1'
def nfet5(s,ref,val,at,d,g,so,lid='Transistor_FET:Q_NMOS_SSSGD_AvalancheRated',fp=MOS5):
    """N-FET by pin name"""
    l=G['libsym'](lid); nets={}
    for u in G['getall'](l,'symbol'):
        for p in G['getall'](u,'pin'):
            nm=str(G['get'](p,'name')[1]); nets[str(G['get'](p,'number')[1])]={'S':so,'G':g,'D':d}[nm[0]]
    nic(s,lid,ref,val,at,fp,nets)
def XAL(v): return 'Inductor_SMD:L_Coilcraft_'+v
def lm5145(s,x,y,refU,vin,vout,sw,suffix,cfg):
    """one LM5145 block. cfg from parts/lm5145_*.json"""
    t=suffix
    lid=custom('LM5145','lm5145_12v',ren={'15':'EP15','21':'EP'})
    nets={'1':'EN'+t,'2':'RT'+t,'3':'SS'+t,'4':'COMP'+t,'5':'FB'+t,'6':'GND','7':cfg['syncout'],'8':cfg['syncin'],'9':None,'10':cfg['pg'],'11':'ILIM'+t,'12':'GND','13':'LO'+t,'14':'VCC'+t,'15':'GND','16':None,'17':'BST'+t,'18':'HO'+t,'19':sw,'20':vin,'21':'GND'}
    nic(s,lid,refU,'LM5145RGYR',(x+27.94,y+30.48),'Package_DFN_QFN:Texas_RGY_R-PVQFN-N20_EP2.05x3.05mm',nets,ds='https://www.ti.com/lit/ds/symlink/lm5145.pdf')
    nfet5(s,s.nref('Q'),'CSD19534Q5A',(x+78.74,y+15.24),vin,'HO'+t,sw,lid='Transistor_FET:Q_NMOS_SSSGD_AvalancheRated')
    nfet5(s,s.nref('Q'),cfg['q2'],(x+78.74,y+48.26),sw,'LO'+t,'GND')
    g=Grid(s,x+104.14,y+7.62,17.78,27.94,x+420)
    g.L(cfg['l'],sw,cfg.get('lout',vout),XAL(cfg['lfp']))
    g.R(cfg['rrt'],'RT'+t,'GND'); g.R(cfg['rfb1'],vout,'FB'+t); g.R(cfg['rfb2'],'FB'+t,'GND')
    g.R(cfg['rc1'],'COMP'+t,'CC'+t); g.C(cfg['cc1'],'CC'+t,'FB'+t); g.C(cfg['cc2'],'COMP'+t,'FB'+t)
    g.R('100R',vout,'FF'+t); g.C(cfg['cc3'],'FF'+t,'FB'+t)
    g.C('33nF','SS'+t,'GND'); g.C('100nF 25V','BST'+t,'BSW'+t); g.R('2.2R','BSW'+t,sw); g.C('2.2uF 25V','VCC'+t,'GND',C603)
    g.R(cfg['rilim'],'ILIM'+t,sw); g.C(cfg['cilim']+' C0G 100V','ILIM'+t,'GND',C603)
    g.R(cfg['ruv1'],vin,'EN'+t,'Resistor_SMD:R_0805_2012Metric'); g.R(cfg['ruv2'],'EN'+t,'GND'); g.C('100nF 100V',vin,'GND',C603)
    g.R('20k',cfg['pg'],'+3V3_AON')
    g.nl()
    g.C('4.7uF 100V',vin,'GND',C1210,k=cfg['ncin']); g.C(cfg['cout'],vout,'GND',C1210,k=cfg['ncout'])
    nfet5(s,s.nref('Q'),'BSS123',(x+27.94,y+73.66),'EN'+t,cfg['off'],'GND',lid='Transistor_FET:Q_NMOS_GSD',fp='Package_TO_SOT_SMD:SOT-23')
    g2=Grid(s,x+58.42,y+73.66,13.97,27.94,x+95); g2.R('100k',cfg.get('offp','+3V3_AON'),cfg['off'])
    return g
def front_end():
    s=base('front_end_12v_24v','front_end_12v_24v: ideal diode, hold-up, 12 V, 24 V, 3.3 V always-on',1100,'LM5050-1, LM5145 x2, LMR38010. Hand design, values in RATIONALE-log.')
    s.paper='A2'
    GLOBALS.update({'VSYS','+12V','+24V','+3V3_AON','PG_12V','PG_24V','BUCK12_OFF','BUCK24_OFF'})
    nic(s,'Power_Management:LM5050-1','U1101','LM5050MK-1',(50.8,45.72),'Package_TO_SOT_SMD:TSOT-23-6',{'1':'VSYS','2':'GND','3':'GND','4':'VSYS','5':'OR_GATE','6':'VSYS_OR'},ds='https://www.ti.com/lit/ds/symlink/lm5050-1.pdf')
    nfet5(s,s.nref('Q'),'CSD19532Q5B',(99.06,45.72),'VSYS_OR','OR_GATE','VSYS')
    g=Grid(s,127,35.56,17.78,27.94,300); g.C('100nF 100V','VSYS','GND',C805); g.C('100nF 100V','VSYS_OR','GND',C805)
    gh=Grid(s,127,279.4,38.1,30.48,490)
    for _ in range(20): gh.CP('68uF 80V B40950B0686M100','VSYS_OR','GND','Capacitor_SMD:CP_Elec_10x10.5')
    s.text('Hold-up bank, 20 x 68 uF, VSYS_OR to GND',(127,264.16),size=1.5,bold=True)
    s.text('Ideal diode and hold-up. LM5050-1 not LM74700: PLAN 2.3 wants 75 V; LM74700 stops at 65 V, and VSYS needs no reverse-polarity function here.\n20 x 68 uF 80 V hybrid, 10 x 10.8 mm = 1.36 mF, fits the 13 mm board gap: 12.6 ms at 90 W from 46.8 V, 6.8 ms from 37 V. README claimed 1 mF for 10 ms: true only above 48 V.\nThe 24 V converter taps VSYS ahead of the diode so 75 W does not drain the hold-up.',(25.4,15.24),size=1.27)
    lm5145(s,25.4,76.2,'U1102','VSYS_OR','+12V','SW12','_12',dict(syncout='SYNC_24',syncin='VCC_12',pg='PG_12V',q2='BSC040N10NS5ATMA1',l='XAL1510-682MED',lfp='XAL1510-682',rrt='33.2k',rfb1='33.2k',rfb2='2.37k',rc1='6.04k',cc1='10nF',cc2='33pF',cc3='1nF',rilim='348R',cilim='18pF',ruv1='1.21M',ruv2='49.9k',ncin=5,cout='22uF 25V',ncout=7,off='BUCK12_OFF',offp='GND',lout='L12_OUT'))
    g=Grid(s,127+90,35.56,17.78,27.94,520); g.D('BAT54 30V','VCC_12','+12V','Diode_SMD:D_SOD-323'); g.F('VSYS'); g.F('VSYS_OR'); g.F('+12V'); g.F('+24V'); g.F('+3V3_AON')
    lm5145(s,25.4,190.5,'U1103','VSYS','+24V','SW24','_24',dict(syncout=None,syncin='SYNC_24',pg='PG_24V',q2='CSD19534Q5A',l='XAL1510-333MED',lfp='XAL1510-333',rrt='33.2k',rfb1='80.6k',rfb2='2.8k',rc1='17.8k',cc1='6.8nF',cc2='10pF',cc3='680pF',rilim='301R',cilim='22pF',ruv1='499k',ruv2='18.2k',ncin=3,cout='10uF 50V',ncout=7,off='BUCK24_OFF'))
    # LMR38010
    lid=custom('LMR38010','lmr38010_3v3_aon',ren={'9':'EP'})
    nic(s,lid,'U1104','LMR38010SDDAR',(50.8,375.92),'Package_SO:SOIC-8-1EP_3.9x4.9mm_P1.27mm_EP2.95x4.9mm_Mask2.71x3.4mm',{'1':'GND','2':'VSYS_OR','3':'VSYS_OR','4':'RT_AON','5':'FB_AON','6':'PG_AON','7':'BOOT_AON','8':'SW_AON','9':'GND'},ds='https://www.ti.com/lit/ds/symlink/lmr38010.pdf')
    g=Grid(s,104.14,365.76,17.78,27.94,500)
    g.L('XAL6060-223MEC 22uH','SW_AON','+3V3_AON',XAL('XAL6060-XXX')); g.R('100k','+3V3_AON','FB_AON'); g.R('43.2k','FB_AON','GND'); g.R('64.9k','RT_AON','GND'); g.C('4.7uF 100V','VSYS_OR','GND',C1210); g.C('100nF 100V','VSYS_OR','GND',C805); g.C('100nF','BOOT_AON','SW_AON'); g.C('22uF 25V','+3V3_AON','GND',C1210,k=3); g.R('100k','PG_AON','+3V3_AON')
    s.text('3.3 V always-on, on VSYS_OR after the ideal diode so it rides the hold-up bank with compute. EN tied to VIN. Ship mode removes VSYS. 400 kHz, 3.31 V.',(25.4,358.14),size=1.27)
    s.text('BUCK12_OFF: pulled down, so a supervisor reset or hang leaves 12 V, compute and the E-stop loop running. Supervisor drives it high in Standby. BUCK24_OFF: pulled up, supervisor drives low to start 24 V.\nThe 12 V converter is the 300 kHz master; SYNCOUT drives the 24 V converter 180 degrees out of phase. 12 V feeds its own VCC through a Schottky after start-up (lower loss).',(25.4,340),size=1.27)
    return s
def _noop(): pass
def tps56637(s,x,y,ref,t,vout,rtop,lval,lfp,ncout):
    lid=custom('TPS56637','tps56637_7v4')
    nic(s,lid,ref,'TPS56637RPAR',(x+27.94,y+15.24),'marengo:TPS56637_RPA_VQFN-HR-10',{'1':'ENB'+t,'2':'FB'+t,'3':'GND','4':'PG'+t,'5':None,'6':'SW'+t,'7':'BOOT'+t,'8':'+12V','9':'GND','10':None},ds='https://www.ti.com/lit/ds/symlink/tps56637.pdf')
    g=Grid(s,x+73.66,y+3.81,13.97,27.94,x+175)
    g.L(lval,'SW'+t,vout,lfp); g.R(rtop,vout,'FB'+t); g.R('10k','FB'+t,'GND'); g.R('20k',vout,'FF'+t); g.C('220pF C0G','FF'+t,'FB'+t)
    g.C('10uF 50V','+12V','GND',C1206,k=2); g.C('100nF','+12V','GND')
    g.nl(); g.C('100nF','BOOT'+t,'SW'+t); g.C('22uF 25V',vout,'GND',C1210,k=ncout); g.R('169k','+12V','ENB'+t); g.R('36.1k','ENB'+t,'GND'); g.R('100k','PG'+t,'+3V3_AON')
def tps62933(s,x,y,ref,t,vout,rtop,lval,ncout,lid='Regulator_Switching:TPS62933',pn='TPS62933DRLR'):
    nic(s,lid,ref,pn,(x+25.4,y+15.24),'Package_TO_SOT_SMD:SOT-583-8',{'1':None,'2':None,'3':'+12V','4':'GND','5':'SW'+t,'6':'BST'+t,'7':'SS'+t,'8':'FB'+t},ds='https://www.ti.com/lit/ds/symlink/tps62933.pdf')
    g=Grid(s,x+73.66,y+3.81,13.97,27.94,x+175)
    g.L(lval,'SW'+t,vout,XAL('XAL6060-XXX')); g.R(rtop,vout,'FB'+t); g.R('10k','FB'+t,'GND'); g.C('10uF 50V','+12V','GND',C1206,k=2); g.C('100nF','+12V','GND')
    g.nl(); g.C('100nF','BST'+t,'SW'+t); g.C('33nF','SS'+t,'GND'); g.C('22uF 25V',vout,'GND',C1210,k=ncout)
def pol():
    s=base('pol_rails','pol_rails: point-of-load rails from 12 V',1200,'TPS56637 x2, TPS62933 x3, TPS7A20 x2. Hand design, values in RATIONALE-log.')
    s.paper='A2'
    GLOBALS.update({'+12V','+3V3_AON','RAW_7V4','RAW_5V','RAW_3V3','RAW_SPARE','RAW_5V6','+5V_CLEAN','+3V3_CLEAN','EN_CLEAN5','EN_CLEAN3'})
    g=Grid(s,230,160,13.97,27.94,380)
    for f in ('RAW_7V4','RAW_5V','RAW_3V3','RAW_SPARE','RAW_5V6'): g.F(f)
    tps56637(s,25.4,30.48,'U1201','_7V4','RAW_7V4','113k','XAL6060-562MEC 5.6uH',XAL('XAL6060-XXX'),3)
    s.text('Servo rail. Adjust by R top: 90.9k = 6.05 V, 113k = 7.4 V, 130k = 8.4 V. MODE open = FCCM.',(25.4,22.86),size=1.27)
    tps56637(s,25.4,104.14,'U1202','_5V','RAW_5V','73.2k','XAL6030-332MEC 3.3uH',XAL('XAL6030-XXX'),2)
    s.text('5 V peripheral, 4 A. EN divider per TI: starts near 10 V input.',(25.4,96.52),size=1.27)
    tps62933(s,25.4,177.8,'U1203','_3V3','RAW_3V3','31.6k','XAL6060-472MEC 4.7uH',3)
    s.text('3.3 V peripheral, 2 A. RT open = 500 kHz, EN open = on.',(25.4,170.18),size=1.27)
    tps62933(s,25.4,251.46,'U1204','_SPARE','RAW_SPARE','52.3k','XAL6060-682MEC 6.8uH',3)
    s.text('Spare rail, stuffed 5 V. 3.3 V: R top 31.6k. 9 V: see log. Max about 10.5 V (Dmax 0.93 from 12 V), not the 24 V PLAN said.',(25.4,243.84),size=1.27)
    tps62933(s,25.4,325.12,'U1205','_5V6','RAW_5V6','60.4k','XAL6060-682MEC 6.8uH',2,lid='Regulator_Switching:TPS62933F',pn='TPS62933FDRLR')
    s.text('New: 5.6 V pre-regulator. TPS7A20 needs VIN <= 6.0 V and >= VOUT + 0.3 V, so it cannot sit on the 5 V or 3.3 V bucks.',(25.4,317.5),size=1.27)
    X=230
    lid='Regulator_Linear:TPS7A20xxxDBV'
    nic(s,lid,'U1206','TPS7A2050PDBVR',(X+25.4,45.72),'Package_TO_SOT_SMD:SOT-23-5',{'1':'RAW_5V6','2':'GND','3':'EN_CLEAN5','4':None,'5':'+5V_CLEAN'},ds='https://www.ti.com/lit/ds/symlink/tps7a20.pdf')
    g=Grid(s,X+63.5,38.1,13.97,27.94,X+130); g.C('1uF 10V','RAW_5V6','GND',C603); g.C('1uF 10V','+5V_CLEAN','GND',C603)
    nic(s,lid,'U1207','TPS7A2033PDBVR',(X+25.4,106.68),'Package_TO_SOT_SMD:SOT-23-5',{'1':'IN_CLEAN3','2':'GND','3':'EN_CLEAN3','4':None,'5':'+3V3_CLEAN'},ds='https://www.ti.com/lit/ds/symlink/tps7a20.pdf')
    g=Grid(s,X+63.5,99.06,13.97,27.94,X+130); g.R('4.7R 1W','RAW_5V6','IN_CLEAN3','Resistor_SMD:R_2512_6332Metric'); g.C('1uF 10V','IN_CLEAN3','GND',C603); g.C('1uF 10V','+3V3_CLEAN','GND',C603); g.F('IN_CLEAN3')
    s.text('Clean rails, 0.3 A each, under 5 mV ripple. Metered together on RAW_5V6 at rail_switches_meters.',(X+7.62,27.94),size=1.27)
    return s
RAILS=[ # name, bus, straps(A1,A0), shunt, switch, RILIM, R1, R2, R3, CdVdT, input, output
 ('24V','A','GND,GND','10mR','T','4.99k','475k','7.87k','22.1k','22nF','+24V','OUT_24V'),
 ('12V_JETSON','A','GND,VS','10mR','T','6.04k','442k','21.5k','41.2k','47nF','+12V','OUT_12V_JETSON'),
 ('12V_THIRD','A','GND,SDA','10mR','T','6.04k','442k','21.5k','41.2k','47nF','+12V','OUT_12V_THIRD'),
 ('12V_AUX','A','GND,SCL','10mR','T','4.53k','442k','21.5k','41.2k','47nF','+12V','OUT_12V_AUX'),
 ('12V_PI','A','VS,GND','10mR','T','4.99k','442k','21.5k','41.2k','47nF','+12V','OUT_12V_PI_STUB'),
 ('7V4','B','GND,GND','5mR','T','3.01k','383k','54.9k','64.9k','47nF','RAW_7V4','OUT_7V4_SERVO'),
 ('5V','B','GND,VS','5mR','L','','','','','','RAW_5V','OUT_5V_PERIPH'),
 ('3V3','B','GND,SDA','10mR','L','','','','','','RAW_3V3','OUT_3V3_PERIPH'),
 ('SPARE','B','GND,SCL','10mR','T','4.99k','365k','33.2k','97.6k','47nF','RAW_SPARE','OUT_SPARE'),
 ('CLEAN','B','VS,GND','10mR','N','','','','','','RAW_5V6','RAW_5V6_M'),
]
def rails():
    s=base('rail_switches_meters','rail_switches_meters: rail switches and meters',1300,'TPS26631 eFuse / TPS22965 switch + INA238 per output. 2 I2C buses, 11 meters. Hardware 12 V shed.')
    s.paper='A1'; s.refc['#PWR']=5000; s.refc['#FLG']=5000
    GLOBALS.update({'+12V','+24V','+3V3_AON','RAW_7V4','RAW_5V','RAW_3V3','RAW_SPARE','RAW_5V6','RAIL_FLT','I2C_A_SDA','I2C_A_SCL','I2C_B_SDA','I2C_B_SCL','RAIL_ALERT_A','RAIL_ALERT_B'})
    for r in RAILS:
        GLOBALS.add('EN_'+r[0]); GLOBALS.add(r[11])
    GLOBALS.discard('EN_CLEAN'); GLOBALS.discard('RAW_5V6_M')
    for k,r in enumerate(RAILS):
        nm,bus,st,sh,sw,ril,r1,r2,r3,cd,vin,vout=r
        x=25.4+(k%2)*400.05; y=30.48+(k//2)*110.49; t='_'+nm
        s.text(f'{vout}: meter on bus {bus}, straps A1,A0 = {st}, shunt {sh}',(x,y-10.16),size=1.5,bold=True)
        mid='SW'+t
        if sw=='T':
            lid='Power_Management:TPS26631PWP'
            nic(s,lid,f'U{1301+k*2}','TPS26631PWPR',(x+25.4,y+33.02),'Package_SO:HTSSOP-20-1EP_4.4x6.5mm_P0.65mm_EP3.4x6.5mm_Mask2.96x2.96mm_ThermalVias',{'1':vin,'2':vin,'3':vin,'4':None,'5':None,'6':vin,'7':'UV'+t,'8':'OV'+t,'9':'GND','10':'DV'+t,'11':'IL'+t,'12':None,'13':'EN_'+nm,'14':None,'15':'RAIL_FLT','16':'GND','17':None,'18':mid,'19':mid,'20':mid,'21':'GND'},ds='https://www.ti.com/lit/ds/symlink/tps2663.pdf')
            g=Grid(s,x+73.66,y+2.54,13.97,27.94,x+195)
            g.R(ril,'IL'+t,'GND'); g.R(r1,vin,'UV'+t); g.R(r2,'UV'+t,'OV'+t); g.R(r3,'OV'+t,'GND'); g.C(cd,'DV'+t,'GND'); g.C('1uF 50V',vin,'GND',C805); g.C('100nF 50V',vin,'GND'); g.C('100nF 50V',mid,'GND'); g.R('33k','EN_'+nm,'GND')
        elif sw=='L':
            lid=custom('TPS22965','tps22965',ren={'9':'EP'})
            vb=vin if nm=='5V' else 'RAW_5V'
            nic(s,lid,f'U{1301+k*2}','TPS22965DSGR',(x+25.4,y+33.02),'Package_SON:Texas_DSG0008A_WSON-8-1EP_2x2mm_P0.5mm_EP0.9x1.6mm',{'1':vin,'2':vin,'3':'EN_'+nm,'4':vb,'5':'GND','6':'CT'+t,'7':mid,'8':mid,'9':'GND'},ds='https://www.ti.com/lit/ds/symlink/tps22965.pdf')
            g=Grid(s,x+73.66,y+2.54,13.97,27.94,x+195)
            g.C('1uF 10V',vin,'GND',C603); g.C('1nF','CT'+t,'GND'); g.C('10uF 10V',mid,'GND',C603); g.R('100k','EN_'+nm,'GND')
        else:
            g=Grid(s,x+73.66,y+2.54,13.97,27.94,x+195); mid=vin
        g.nl(y+33.02+ -2.54)
        if sw=='L':
            pf={'5V':'2.5A MF-MSMF250/16X-2','3V3':'1.1A MF-MSMF110/16-2'}[nm]
            g.two('Device:Polyfuse','F',pf,mid,mid+'_PF','Fuse:Fuse_1812_4532Metric'); mid=mid+'_PF'
        g.R(sh+' 3W',mid,vout if sw!='N' else 'RAW_5V6_M','Resistor_SMD:R_2512_6332Metric')
        out=vout
        g.R('10R',mid,'SP'+t); g.R('10R',out,'SN'+t); g.C('100nF','SP'+t,'SN'+t); g.C('100nF','+3V3_AON','GND')
        g.nl(y+60.96)
        sda,scl=f'I2C_{bus}_SDA',f'I2C_{bus}_SCL'
        A1,A0=[{'GND':'GND','VS':'+3V3_AON','SDA':sda,'SCL':scl}[v] for v in st.split(',')]
        nic(s,'Sensor_Energy:INA238',f'U{1302+k*2}','INA238AIDGSR',(x+25.4+150,y+80),'Package_SO:TSSOP-10_3x3mm_P0.5mm',{'1':A1,'2':A0,'3':'RAIL_ALERT_'+bus,'4':sda,'5':scl,'6':'+3V3_AON','7':'GND','8':out,'9':'SN'+t,'10':'SP'+t},ds='https://www.ti.com/lit/ds/symlink/ina238.pdf')
    # 12 V total meter and hardware shed
    y=60.96
    s.text('+12V total: converter output to rail, bus A, straps VS,VS.\nALERT at 12.0 A, latched, 150 us conversion, sheds third board,\naux, spare in hardware. Firmware clears the latch.',(251.46,y-27.94),size=1.5,bold=True)
    g=Grid(s,251.46,y+2.54,13.97,27.94,330)
    g.R('2mR 3W','L12_OUT','+12V','Resistor_SMD:R_2512_6332Metric'); g.R('10R','L12_OUT','SP_T12'); g.R('10R','+12V','SN_T12'); g.C('100nF','SP_T12','SN_T12'); g.C('100nF','+3V3_AON','GND'); g.R('10k','SHED_N','+3V3_AON')
    nic(s,'Sensor_Energy:INA238','U1321','INA238AIDGSR',(274.32,y+60.96),'Package_SO:TSSOP-10_3x3mm_P0.5mm',{'1':'+3V3_AON','2':'+3V3_AON','3':'SHED_N','4':'I2C_A_SDA','5':'I2C_A_SCL','6':'+3V3_AON','7':'GND','8':'+12V','9':'SN_T12','10':'SP_T12'},ds='https://www.ti.com/lit/ds/symlink/ina238.pdf')
    nic(s,'74xGxx:74LVC1G14','U1322','SN74LVC1G14DBV',(340.36,y+60.96),'Package_TO_SOT_SMD:SOT-23-5',{'1':None,'2':'SHED_N','3':'GND','4':'SHED_12V','5':'+3V3_AON'})
    for k,e in enumerate(('EN_12V_THIRD','EN_12V_AUX','EN_SPARE')):
        nic(s,'Transistor_FET:Q_NMOS_GSD',s.nref('Q'),'2N7002',(264.16+k*40.64,y+106.68),'Package_TO_SOT_SMD:SOT-23',{'1':'SHED_12V','2':'GND','3':e})
    # single-ended pull-ups
    g=Grid(s,25.4,590,13.97,27.94,300)
    g.R('100k','RAIL_FLT','+3V3_AON'); g.R('10k','RAIL_ALERT_A','+3V3_AON'); g.R('10k','RAIL_ALERT_B','+3V3_AON')
    s.text('TPS26631: UV/OV chain mandatory (UVLO to GND means 15.46 V, would kill a 12 V rail). Latch mode, supervisor clears. SHDN default off.\nTPS22965 has no current limit, its 6 A is under the buck limit: polyfuse after each switch, 5 V 2.5 A, 3.3 V 1.1 A.\nOutput limits: 24 V 3.61 A, Jetson 2.98 A, third 2.98 A, aux 3.97 A, Pi stub 3.61 A, 7.4 V 5.98 A, spare 3.61 A (TPS26631 latch off). 5 V 2.5 A, 3.3 V 1.1 A (polyfuse, hold at 23 C, derates hot).\nINA238 has no energy registers: firmware integrates at 100 Hz, or swap to INA228 (same footprint).',(25.4,572),size=1.5)
    return s
