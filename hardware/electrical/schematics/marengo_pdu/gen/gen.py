import sys,uuid,math,copy,re,os
sys.path.insert(0,os.path.dirname(os.path.abspath(__file__)))
sys.path.insert(0,os.path.join(os.path.dirname(os.path.abspath(__file__)),'..','tools'))
from sx import *
LIB='/Applications/KiCad/KiCad.app/Contents/SharedSupport/symbols/'
OUT=sys.argv[1]
PROJ='marengo_pdu'; NS=uuid.UUID('6f3e1f0a-3c55-4f0e-9d3a-6d2b1d4c5a10')
ROOT='a840985c-3387-42ba-bc64-1037eae04a32'
def U(s): return str(uuid.uuid5(NS,s))
FONT=[ 'font',['size','1.27','1.27']]
n=lambda v:('%.4f'%v).rstrip('0').rstrip('.')
def eff(j=None,hide=False,bold=False,size=1.27):
    e=['effects',['font',['size',n(size),n(size)]]+([['bold','yes']] if bold else [])]
    if j: e.append(['justify']+j.split())
    if hide: e.append(['hide','yes'])
    return e
# ---------- BQ76952 project symbol ----------
L=[('BAT',47,'power_in'),('CP1',46,'bidirectional'),None]+[('VC16',48,'input')]+[(f'VC{15-i}',i+1,'input') for i in range(16)]+[None,('SRP',18,'input'),('SRN',20,'input'),None,('VSS',17,'power_in'),('NC',19,'no_connect'),('NC',44,'no_connect')]
Rp=[('CHG',45,'output'),('DSG',43,'output'),('PCHG',40,'output'),('PDSG',39,'output'),('PACK',42,'input'),('LD',41,'bidirectional'),('FUSE',38,'bidirectional'),None,
    ('BREG',37,'passive'),('REGIN',36,'passive'),('REG1',35,'power_out'),('REG2',34,'power_out'),('REG18',24,'power_out'),None,
    ('SDA',27,'bidirectional'),('SCL',26,'bidirectional'),('ALERT',25,'bidirectional'),('RST_SHUT',33,'input'),('DFETOFF',30,'bidirectional'),None,
    ('CFETOFF',29,'bidirectional'),('HDQ',28,'bidirectional'),('DCHG',31,'bidirectional'),('DDSG',32,'bidirectional'),('TS1',21,'bidirectional'),('TS2',22,'bidirectional'),('TS3',23,'bidirectional')]
assert len(L)==27 and len(Rp)==27
PINXY={}
def mkbq():
    pins=[]
    for side,lst in (('L',L),('R',Rp)):
        for k,p in enumerate(lst):
            if not p: continue
            y=33.02-2.54*k; x=-25.4 if side=='L' else 25.4
            nm,num,tp=p; PINXY[str(num)]=(x,y); PINXY[nm]=(x,y)
            pins.append(['pin',tp,'line',['at',n(x),n(y),'0' if side=='L' else '180'],['length','5.08'],['name',S(nm),eff()],['number',S(str(num)),eff()]])
    assert sorted(int(p[1]) for lst in (L,Rp) for p in lst if p)==list(range(1,49))
    def prop(k,v,y,hide=True): return ['property',S(k),S(v),['at','0',n(y),'0'],eff(hide=hide)]
    return ['symbol',S('BQ76952'),['pin_names',['offset','1.016']],['exclude_from_sim','no'],['in_bom','yes'],['on_board','yes'],
      prop('Reference','U',36.83,False),prop('Value','BQ76952',-36.83,False),
      prop('Footprint','Package_QFP:TQFP-48_7x7mm_P0.5mm',-39.37),prop('Datasheet','https://www.ti.com/lit/ds/symlink/bq76952.pdf',-41.91),
      prop('Description','TI 3S-16S battery monitor/protector, I2C default, 48-pin TQFP (PFB). Pinout from SLUSE13B Table 6-1.',-44.45),
      ['property',S('ki_keywords'),S('battery AFE monitor protector BMS lithium'),['at','0','0','0'],eff(hide=True)],
      ['symbol',S('BQ76952_0_1'),['rectangle',['start','-20.32','35.56'],['end','20.32','-35.56'],['stroke',['width','0.254'],['type','default']],['fill',['type','background']]]],
      ['symbol',S('BQ76952_1_1')]+pins,['embedded_fonts','no']]
BQ=mkbq()
CUSTOM={}
def mkic(name,pins,fp='',ds='',desc='',ref='U'):
    """box symbol from a datasheet pin table: pins=[(num,name,etype)]. inputs left, outputs right, power on the side they fall."""
    left=[p for p in pins if p[2] in('input','power_in')]; right=[p for p in pins if p not in left]
    if abs(len(left)-len(right))>6:
        al=left+right; h=(len(al)+1)//2; left,right=al[:h],al[h:]
    h=max(len(left),len(right)); y0=2.54*(h-1)/2; y0=round(y0/2.54)*2.54
    w=max([len(p[1]) for p in pins])*1.27+5.08; w=max(10.16,round(w/2.54)*2.54)
    ps=[]
    for lst,x,ang in ((left,-w-2.54-5.08+2.54,0),(right,w+5.08,180)):
        for k,(num,nm,tp) in enumerate(lst):
            ps.append(['pin',tp,'line',['at',n(-w-5.08 if ang==0 else w+5.08),n(y0-2.54*k),str(ang)],['length','5.08'],['name',S(nm),eff()],['number',S(str(num)),eff()]])
    def prop(k,v,y,hide=True): return ['property',S(k),S(v),['at','0',n(y),'0'],eff(hide=hide)]
    top=y0+2.54; bot=y0-2.54*h
    sym=['symbol',S(name),['pin_names',['offset','1.016']],['exclude_from_sim','no'],['in_bom','yes'],['on_board','yes'],
      prop('Reference',ref,top+2.54,False),prop('Value',name,bot-2.54,False),prop('Footprint',fp,bot-5.08),prop('Datasheet',ds or '~',bot-7.62),prop('Description',desc,bot-10.16),
      ['symbol',S(name+'_0_1'),['rectangle',['start',n(-w),n(top)],['end',n(w),n(bot)],['stroke',['width','0.254'],['type','default']],['fill',['type','background']]]],
      ['symbol',S(name+'_1_1')]+ps,['embedded_fonts','no']]
    CUSTOM[name]=sym; return sym
def writelib():
    open(OUT+'/marengo.kicad_sym','w').write(dump(['kicad_symbol_lib',['version','20251024'],['generator',S('kicad_symbol_editor')],['generator_version',S('10.0')],BQ]+[CUSTOM[k] for k in sorted(CUSTOM)])+'\n')
open(OUT+'/sym-lib-table','w').write('(sym_lib_table\n\t(version 7)\n\t(lib (name "marengo")(type "KiCad")(uri "${KIPRJMOD}/marengo.kicad_sym")(options "")(descr "marengo project symbols"))\n)\n')
# ---------- library access ----------
_c={}
def libsym(lid):
    lib,name=lid.split(':')
    if lib=='marengo': s=copy.deepcopy(BQ if name=='BQ76952' else CUSTOM[name])
    else:
        if lib not in _c: _c[lib]={s[1]:s for s in getall(parse(open(LIB+lib+'.kicad_sym').read()),'symbol')}
        s=copy.deepcopy(_c[lib][name])
    ext=get(s,'extends')
    if ext:
        p=copy.deepcopy(_c[lib][str(ext[1])]); child=str(s[1]); pn=str(ext[1])
        over={str(x[1]):x for x in getall(s,'property')}
        for i,x in enumerate(p):
            if isinstance(x,list) and x and x[0]=='property' and str(x[1]) in over: p[i]=over[str(x[1])]
            if isinstance(x,list) and x and x[0]=='symbol' and str(x[1]).startswith(pn+'_'): x[1]=S(child+str(x[1])[len(pn):])
        s=p
    s[1]=S(lid); return s
def pins_of(s):
    out={}
    for u in getall(s,'symbol'):
        for p in getall(u,'pin'):
            a=get(p,'at'); out[str(get(p,'number')[1])]=(float(a[1]),float(a[2]))
    return out
def pos(sym,pin,at,rot):
    px,py=pins_of(sym)[pin]; r=math.radians(rot)
    x=px*math.cos(r)-py*math.sin(r); y=px*math.sin(r)+py*math.cos(r)
    return (round(at[0]+x,4),round(at[1]-y,4))
class Sheet:
    def __init__(s,key,title,paper='A3',comment=None):
        s.key=key;s.title=title;s.paper=paper;s.items=[];s.lib={};s.c=0;s.comment=comment;s.refc={}
        s.path=None
    def id(s,t='x'):
        s.c+=1; return U(f'{s.key}/{t}/{s.c}')
    def wire(s,*pts):
        for a,b in zip(pts,pts[1:]):
            s.items.append(['wire',['pts',['xy',n(a[0]),n(a[1])],['xy',n(b[0]),n(b[1])]],['stroke',['width','0'],['type','default']],['uuid',S(s.id())]])
    def junc(s,p): s.items.append(['junction',['at',n(p[0]),n(p[1])],['diameter','0'],['color','0','0','0','0'],['uuid',S(s.id())]])
    def nc(s,p): s.items.append(['no_connect',['at',n(p[0]),n(p[1])],['uuid',S(s.id())]])
    def label(s,t,p,ang=0):
        s.items.append(['label',S(t),['at',n(p[0]),n(p[1]),str(ang)],eff('right bottom' if ang in(180,270) else 'left bottom'),['uuid',S(s.id())]])
    def hier(s,t,p,ang=0,shape='passive'):
        s.items.append(['hierarchical_label',S(t),['shape',shape],['at',n(p[0]),n(p[1]),str(ang)],eff('right' if ang==180 else 'left'),['uuid',S(s.id())]])
    def glabel(s,t,p,ang=0):
        s.items.append(['global_label',S(t),['shape','passive'],['at',n(p[0]),n(p[1]),str(ang)],['fields_autoplaced','yes'],eff('right' if ang in(180,270) else 'left'),['uuid',S(s.id())],['property',S('Intersheetrefs'),S('${INTERSHEET_REFS}'),['at',n(p[0]),n(p[1]),'0'],eff(hide=True)]])
    def text(s,t,p,size=1.27,bold=False):
        s.items.append(['text',S(t),['exclude_from_sim','no'],['at',n(p[0]),n(p[1]),'0'],eff('left top',size=size,bold=bold),['uuid',S(s.id())]])
    def sym(s,lid,ref,val,at,rot=0,fp='',unit=1,refat=None,valat=None,hideval=False,hideref=False,ds=''):
        l=libsym(lid); s.lib[lid]=l
        pw=lid.startswith('power:')
        rtext=90 if rot in(90,270) else 0
        r=refat or (at[0]+(0 if rot in(90,270) else 3.81), at[1]-(2.54 if rot in(90,270) else 1.27))
        v=valat or (at[0]+(0 if rot in(90,270) else 3.81), at[1]+(2.54 if rot in(90,270) else 1.27))
        just=None if rot in(90,270) else 'left'
        if pw: just=None
        pr=lambda k,t,p,hide=False,j=just:['property',S(k),S(t),['at',n(p[0]),n(p[1]),str(rtext)],eff(j,hide=hide)]
        o=['symbol',['lib_id',S(lid)],['at',n(at[0]),n(at[1]),str(rot)],['unit','1'],['exclude_from_sim','no'],['in_bom','no' if pw else 'yes'],['on_board','no' if pw else 'yes'],['dnp','no'],['uuid',S(U(f'{s.key}/sym/{ref}'))],
           pr('Reference',ref,r,hide=pw or hideref),pr('Value',val,v,hide=hideval or lid=='power:PWR_FLAG'),pr('Footprint',fp,at,True),pr('Datasheet',ds or '~',at,True),pr('Description','',at,True)]
        for p in pins_of(l): o.append(['pin',S(p),['uuid',S(U(f'{s.key}/pin/{ref}/{p}'))]])
        o.append(['__inst__',ref])
        s.items.append(o); return l
    def nref(s,pfx):
        s.refc[pfx]=s.refc.get(pfx,0)+1; return f'{pfx}{s.refc[pfx]}'
    def write(s,path_uuid_list):
        # path_uuid_list: list of (instance path string, page)
        items=[]
        for it in s.items:
            if it[0]=='symbol':
                it=copy.deepcopy(it); ref=it.pop()[1]
                def rr(i):
                    if i==0 or not getattr(s,'multi',0): return ref
                    m=re.match(r'(#?[A-Z]+)(\d+)$',ref); return m[1]+str(int(m[2])+i*s.multi)
                it.append(['instances',['project',S(PROJ)]+[['path',S(p),['reference',S(rr(i))],['unit','1']] for i,(p,_) in enumerate(path_uuid_list)]])
            items.append(it)
        tb=['title_block',['title',S(s.title)],['date',S('2026-11-06')],['rev',S('A-wip')],['company',S('marengo')]]
        if s.comment: tb.append(['comment','1',S(s.comment)])
        doc=['kicad_sch',['version','20260306'],['generator',S('eeschema')],['generator_version',S('10.0')],['uuid',S(ROOT if s.key=='root' else U('file/'+s.key))],['paper',S(s.paper)],tb,['lib_symbols']+list(s.lib.values())]+items
        if s.key=='root': doc.append(['sheet_instances',['path',S('/'),['page',S('1')]]])
        doc.append(['embedded_fonts','no'])
        open(f'{OUT}/{s.file}','w').write(dump(doc)+'\n')
def sheetsym(par,child_name,file,at,size,pins,inst,fid,dnp=False):
    """pins: names on the right edge"""
    x,y=at;w,h=size
    o=['sheet',['at',n(x),n(y)],['size',n(w),n(h)],['exclude_from_sim','no'],['in_bom','yes'],['on_board','yes'],['dnp','yes' if dnp else 'no'],['stroke',['width','0.1524'],['type','solid']],['fill',['color','0','0','0','0.0000']],['uuid',S(fid)],
       ['property',S('Sheetname'),S(child_name),['at',n(x),n(y-0.7116),'0'],eff('left bottom')],
       ['property',S('Sheetfile'),S(file),['at',n(x),n(y+h+0.5846),'0'],eff('left top')]]
    for k,(p,tp) in enumerate(pins):
        o.append(['pin',S(p),tp,['at',n(x+w),n(y+2.54+2.54*k),'0'],['uuid',S(U(f'{fid}/{p}'))],eff('right')])
    o.append(['instances',['project',S(PROJ),['path',S(inst[0]),['page',S(inst[1])]]]])
    par.items.append(o)
    return [(x+w,y+2.54+2.54*k) for k in range(len(pins))]
# ---------- pack_input_shunt ----------
P=Sheet('pack','pack_input_shunt_afe',comment='Pack terminals, sense header, shunt, BQ76952 AFE. Part numbers verify. TI SLUSE13B.')
P.file='pack_input_shunt_afe.kicad_sch'
Rr,Cc=(lambda:P.nref('R')),(lambda:P.nref('C'))
R603='Resistor_SMD:R_0603_1608Metric';C603='Capacitor_SMD:C_0603_1608Metric'
def vss(p,rot=180): P.sym('power:VSS',P.nref('#PWR'),'VSS',p,rot)
def rh(at,val,fp=R603,**k):
    ref=Rr(); P.sym('Device:R',ref,val,at,90,fp,**k); return (at[0]-3.81,at[1]),(at[0]+3.81,at[1])
def cv(at,val,fp=C603,**k):
    ref=Cc(); P.sym('Device:C',ref,val,at,0,fp,**k); return (at[0],at[1]-3.81),(at[0],at[1]+3.81)
def ch(at,val,fp=C603,**k):
    ref=Cc(); P.sym('Device:C',ref,val,at,90,fp,**k); return (at[0]-3.81,at[1]),(at[0]+3.81,at[1])
# J2 Micro-Fit
J2=(58.42,60.96)
P.sym('Connector_Generic:Conn_02x10_Top_Bottom','J2','2x10 locking header',J2,0,'Connector_Molex:Molex_Micro-Fit_3.0_43045-2012_2x10_P3.00mm_Vertical',refat=(J2[0],J2[1]-16.51),valat=(J2[0],J2[1]-13.97))
jl=libsym('Connector_Generic:Conn_02x10_Top_Bottom')
PIN={1:'NTC1',11:'NTC2',2:'NTC3',12:None,3:'VSS',13:'PACK_PRESENT_N',4:'B0',14:'B1',5:'B2',15:'B3',6:'B4',16:'B5',7:'B6',17:'B7',8:'B8',18:'B9',9:'B10',19:'B11',10:'B12',20:'B13'}
for num,net in PIN.items():
    p=pos(jl,str(num),J2,0); left=num<=10
    e=(p[0]+(-5.08 if left else 5.08),p[1]); 
    if net is None: P.nc(p); continue
    P.wire(p,e)
    if net=='VSS': vss(e,90)
    elif net.startswith('NTC') or net=='PACK_PRESENT_N': P.hier(net,e,180 if left else 0)
    else: P.label(net,e,180 if left else 0)
P.text('J2: harness to the pack sense board 2x10, same pin map.\nColumns 4-10 zigzag B0..B13: a bent pin shorts 1 cell\nacross a column, 2 cells along a row.\nPin 3: NTC and presence return, to VSS here, no cell connection.\nPin 4: B0 Kelvin tap, no bond to BAT- (R below). Spare: pin 12.',(25.4,76.2))
# pack power terminals
l1=libsym('Connector_Generic:Conn_01x01')
J1=(58.42,96.52); J3=(58.42,104.14)
P.sym('Connector_Generic:Conn_01x01','J1','M4 terminal, REDCUBE 74651194R',J1,0,'TerminalBlock_Wuerth:Wuerth_REDCUBE-THR_WP-THRSH_74651194_THR',refat=(J1[0]-5.08,J1[1]-2.54),valat=(J1[0]+5.08,J1[1]))
P.sym('Connector_Generic:Conn_01x01','J3','M4 terminal, REDCUBE 74651194R',J3,0,'TerminalBlock_Wuerth:Wuerth_REDCUBE-THR_WP-THRSH_74651194_THR',refat=(J3[0]-5.08,J3[1]-2.54),valat=(J3[0]+5.08,J3[1]))
a=pos(l1,'1',J1,0); b=pos(l1,'1',J3,0)
P.wire(a,(a[0]-5.08,a[1])); P.hier('PACK_TOP',(a[0]-5.08,a[1]),180)
P.text('J1 pack +, J3 pack -: 10 AWG ring-lug pigtails to the XT90 at the pack.',(25.4,90.17))
# shunt on pack negative
RS=(45.72,121.92); ls=libsym('Device:R_Shunt')
P.sym('Device:R_Shunt','RS1','0.3mR CSS4J-4026 5W',RS,0,'Resistor_SMD:R_Shunt_Isabellenhuette_BVR4026',refat=(RS[0]+6.35,RS[1]+6.35),valat=(RS[0]+6.35,RS[1]+8.89))
s1,s2,s3,s4=[pos(ls,x,RS,0) for x in '1234']
T=(45.72,109.22)
P.wire(b,(45.72,b[1]),T); P.wire(T,s1); P.junc(T); P.label('BAT-',(45.72,106.68))
P.wire(s2,(s2[0]+5.08,s2[1])); P.label('SRP',(s2[0]+5.08,s2[1]))
P.wire(s3,(s3[0]+5.08,s3[1])); P.label('SRN',(s3[0]+5.08,s3[1]))
G=(45.72,132.08); P.wire(s4,G); P.sym('power:GND',P.nref('#PWR'),'GND',G); P.junc((45.72,129.54))
P.wire((45.72,129.54),(40.64,129.54)); 
P.sym('power:PWR_FLAG','#FLG1','PWR_FLAG',(40.64,129.54))
NT=(55.88,109.22); ln=libsym('Device:NetTie_2')
P.sym('Device:NetTie_2','NT1','Net-Tie',NT,0,'NetTie:NetTie-2_SMD_Pad0.5mm',refat=(NT[0],NT[1]-2.54),valat=(NT[0],NT[1]+2.54))
t1=pos(ln,'1',NT,0); t2=pos(ln,'2',NT,0)
P.wire(T,t1); V=(63.5,109.22); P.wire(t2,V); vss((68.58,109.22),270); P.wire(V,(68.58,109.22)); P.junc(V)
P.sym('power:PWR_FLAG','#FLG2','PWR_FLAG',V)
P.text('Pack negative -> shunt -> system GND. SRP is the cell side:\ncharge current makes SRP positive (datasheet p5).\nAFE VSS ties at BAT-, so VSS sits above GND by I x 0.3 mR\nin discharge: 60 mV at the 200 A SCD point.',(25.4,137.16))
# cell filter
ROWX=(88.9,97.79,105.41,111.76,116.84); Y=lambda i:38.1+(13-i)*10.16
NODE={13:'VC16',12:'VC12'}
for i in range(13,-1,-1):
    y=Y(i); P.wire((ROWX[0],y),(97.79,y)); P.label(f'B{i}',(ROWX[0],y),180)
    rh((101.6,y),'20R' if i==13 else '100R')
    P.wire((105.41,y),(111.76,y)); P.wire((111.76,y),(116.84,y)); P.label(NODE.get(i,f'VC{i}'),(116.84,y))
    if i>0:
        a_,b_=cv((111.76,y+5.08),'220nF',refat=(115.57,y+3.81),valat=(115.57,y+6.35))
        P.wire((111.76,y),a_); P.wire(b_,(111.76,y+10.16))
    if 0<i<13: P.junc((111.76,y))
P.text('Cell filter: 100R + 220nF, 20R on VC16 (SLUAA81 2.5, top cell IR drop)\n(datasheet p9: RC 20-100R, CC 0.1/0.22/1 uF). 100R gives the balance FET its gate drive.',(88.9,175.26))
# U1
UC=(190.5,101.6)
P.sym('marengo:BQ76952','U1','BQ76952PFBR',UC,0,'Package_QFP:TQFP-48_7x7mm_P0.5mm',refat=(UC[0]-3.81,UC[1]-36.83),valat=(UC[0]-3.81,UC[1]+37.846),ds='https://www.ti.com/lit/ds/symlink/bq76952.pdf')
def pin(nm):
    x,y=PINXY[nm]; return (UC[0]+x,UC[1]-y)
for i in list(range(0,12))+[16]:
    p=pin(f'VC{i}'); e=(p[0]-7.62,p[1]); P.wire(p,e); P.label(f'VC{i}',e,180)
# VC12..VC15 shorted
bx=pin('VC12')[0]-5.08
ys=[pin(f'VC{i}') for i in (15,14,13,12)]
for p in ys: P.wire(p,(bx,p[1]))
for a_,b_ in zip(ys,ys[1:]): P.wire((bx,a_[1]),(bx,b_[1]))
for p in ys[1:3]: P.junc((bx,p[1]))
e=(pin('VC12')[0]-7.62,pin('VC12')[1]); P.wire((bx,ys[3][1]),e); P.label('VC12',e,180); P.junc((bx,ys[3][1]))
P.text('13S per Table 16-3 (p75): cells 1, 2, 16 must be real.\nVC0..VC12 = cells 1-12, VC15-VC16 = cell 13.\nVC13-15 short to VC12. Cell inputs in use: 1-12 and 16.',(122.92,146.05))
for nm,net in (('SRP','SRP_F'),('SRN','SRN_F'),('BAT','BAT'),('CP1','CP1')):
    p=pin(nm); e=(p[0]-5.08,p[1]); P.wire(p,e); P.label(net,e,180)
p=pin('VSS'); e=(p[0]-5.08,p[1]); P.wire(p,e); vss(e,90)
# BAT network
y=33.02; P.hier('PACK_TOP',(129.54,y),180); ld=libsym('Device:D')
P.wire((129.54,y),(133.35,y))
P.sym('Device:D','D1','BAV19W 120V',(137.16,y),180,'Diode_SMD:D_SOD-123',refat=(137.16,y-2.54),valat=(137.16,y+2.54))
P.wire((140.97,y),(142.24,y)); rh((146.05,y),'100R',refat=(146.05,y-2.54),valat=(146.05,y+2.54))
N=[(154.94,y),(165.1,y),(175.26,y),(182.88,y),(187.96,y)]
P.wire((149.86,y),N[0])
for a_,b_ in zip(N,N[1:]): P.wire(a_,b_)
for q in N[:4]: P.junc(q)
P.label('BAT',N[4]); P.sym('power:PWR_FLAG','#FLG3','PWR_FLAG',N[3])
Yc=y+7.62
a_,b_=cv((154.94,Yc),'1uF 100V','Capacitor_SMD:C_1206_3216Metric',refat=(156.21,Yc-1.27),valat=(156.21,Yc+1.27)); P.wire(N[0],a_); vss(b_)
a_,b_=cv((165.1,Yc),'100pF',refat=(166.37,Yc-1.27),valat=(166.37,Yc+1.27)); P.wire(N[1],a_); vss(b_)
a_,b_=cv((175.26,Yc),'1uF',refat=(176.53,Yc-1.27),valat=(176.53,Yc+1.27)); P.wire(N[2],a_); P.wire(b_,(175.26,46.99)); P.label('CP1',(175.26,46.99))
P.text('BAT from the pack + terminal, so the AFE stays up if J2 is pulled\nwith pack power still connected. Fig 16-2: diode, 100R, 1uF, 100pF. CP1 is 1uF, not 0.47uF: see sheet bms_fets_precharge. Caps 100 V.',(129.54,22.86))
# SRP/SRN filter
y1,y2=185.42,198.12
for lab,yy in (('SRP',y1),('SRN',y2)):
    P.wire((127,yy),(133.35,yy)); P.label(lab,(127,yy),180)
    rh((137.16,yy),'100R',refat=(137.16,yy-2.54),valat=(137.16,yy+2.54))
    P.wire((140.97,yy),(146.05,yy)); P.wire((146.05,yy),(153.67,yy)); P.wire((153.67,yy),(160.02,yy)); P.label(lab+'_F',(160.02,yy))
    P.junc((146.05,yy)); P.junc((153.67,yy))
a_,b_=cv((146.05,191.77),'100nF',refat=(134.62,190.5),valat=(134.62,193.04)); P.wire((146.05,y1),a_); P.wire(b_,(146.05,y2))
a_,b_=cv((153.67,191.77),'100pF',refat=(156.21,190.5),valat=(156.21,193.04)); P.wire((153.67,y1),a_); P.wire(b_,(153.67,y2))
P.text('p65, p76: 100R each side, 100nF + 100pF differential, near U1,\nsymmetric, same PCB side. Kelvin SRP/SRN routed in parallel from RS1.',(127,204.47))
# right side of U1
def hr(nm,net,shape='passive'):
    p=pin(nm); e=(p[0]+7.62,p[1]); P.wire(p,e); P.hier(net,e,0,shape)
hr('CHG','CHG_GATE','output'); hr('DSG','DSG_GATE','output'); hr('PDSG','PDSG','output')
for nm in ('PCHG','FUSE','DCHG','DDSG'): P.nc(pin(nm))
hr('SDA','I2C_SDA','bidirectional'); hr('SCL','I2C_SCL','bidirectional'); hr('ALERT','ALERT','output'); hr('RST_SHUT','RST_SHUT','input'); hr('DFETOFF','DFETOFF','input')
hr('CFETOFF','NTC3','input'); hr('HDQ','BOARD_NTC','input'); hr('TS1','NTC1','input'); hr('TS3','NTC2','input')
for nm,net in (('PACK','PACK_S'),('LD','LD_S'),('TS2','TS2_F')):
    p=pin(nm); e=(p[0]+7.62,p[1]); P.wire(p,e); P.label(net,e)
# REG terminations
bx=pin('BREG')[0]+5.08
ys=[pin(k) for k in ('BREG','REGIN')]
for p in ys: P.wire(p,(bx,p[1]))
for a_,b_ in zip(ys,ys[1:]): P.wire((bx,a_[1]),(bx,b_[1]))
P.junc((bx,ys[1][1])); e=(bx+2.54,ys[1][1]); P.wire((bx,ys[1][1]),e); vss(e,270)
P.nc(pin('REG1')); P.nc(pin('REG2'))
p=pin('REG18'); e=(p[0]+10.16,p[1]); P.wire(p,e); a_,b_=ch((e[0]+3.81,e[1]),'2.2uF',refat=(e[0]+3.81,e[1]-2.54),valat=(e[0]+3.81,e[1]+2.54)); P.wire(b_,(b_[0]+5.08,b_[1])); vss((b_[0]+5.08,b_[1]),270)
# right blocks
for lab,yy in (('PACK_S',60.96),('LD_S',71.12)):
    P.wire((256.54,yy),(260.35,yy)); P.label(lab,(256.54,yy),180)
    rh((264.16,yy),'5.1k',refat=(264.16,yy-2.54),valat=(264.16,yy+2.54)); P.wire((267.97,yy),(271.78,yy))
    rh((275.59,yy),'5.1k',refat=(275.59,yy-2.54),valat=(275.59,yy+2.54)); P.wire((279.4,yy),(284.48,yy)); P.hier('VSYS',(284.48,yy),0)
P.text('PACK and LD both sense VSYS, the common port after the FETs\n(Fig 16-3 R42-R45). PCHG, FUSE, DCHG, DDSG unused, left floating (p75).',(256.54,47.0))
yy=83.82; P.hier('RST_SHUT',(256.54,yy),180,'input'); P.wire((256.54,yy),(260.35,yy)); rh((264.16,yy),'1k',refat=(264.16,yy-2.54),valat=(264.16,yy+2.54)); vss((269.24,yy),270); P.wire((267.97,yy),(269.24,yy))
yy=93.98; P.label('TS2_F',(256.54,yy),180); P.wire((256.54,yy),(260.35,yy)); rh((264.16,yy),'10k',refat=(264.16,yy-2.54),valat=(264.16,yy+2.54)); P.wire((267.97,yy),(271.78,yy)); P.hier('TS2_WAKE',(271.78,yy),0,'input')
for k,lab in enumerate(('NTC1','NTC2','NTC3')):
    yy=106.68+7.62*k; P.hier(lab,(256.54,yy),180,'input'); P.wire((256.54,yy),(260.35,yy)); a_,b_=ch((264.16,yy),'1nF',refat=(264.16,yy-2.54),hideval=(k>0)); 
    P.wire((267.97,yy),(274.32,yy)); vss((274.32,yy),270)
P.text('Four thermistors: TS1, TS3, CFETOFF (NTC1-3), HDQ (board).\nTS2 stays free as SHUTDOWN wake (p22 pin table, section 13):\nswitch to VSS through 10k (Fig 16-2 R27). 1nF: p65 limit <4nF.\nREGIN, BREG tied to VSS, REG1, REG2 open (p75): I2C pull-ups on LP.\nCFETOFF as NTC needs DFETOFF set as BOTHOFF to stop CHG by pin (p48).',(256.54,127.0))
# Notes
NOTE1='''SIZING (shunt 0.3 mOhm: no 0.25 mOhm 4-terminal part exists; datasheet p22 steps)
OCD1 24 mV = 80 A, 66.7-93.3 A at +/-4 mV, delay 100 ms    OCD3 60 A for 1 s (current-based, inside the AFE, the agreed 60 A ceiling)
OCC 12 mV = 40 A, 33.3-48.8 A at -2/+2.65 mV, under the 50 A limit    SCD 60 mV = 200 A, 170-230 A, delay 30 us: bank A short protection (105-165 A, about 10 us) acts first
OCD and OCC are set in 2 mV steps. V(OC_ACC) is -2/+2.65 mV below 20 mV, +/-4 mV 20-56 mV, +/-5 mV 56-100 mV. RS1 takes 6.75 W at 150 A: 5 W part, 100 ms path only.'''
NOTE2='''OPEN ON THIS SHEET
1. Balance FETs are captured below (SLUAA81 Fig 2-2). OTP/data memory owed: Cell Balance Min Cell V 3900 mV, Max Cells 4.
2. CP1 raised to 1 uF for the 4+4 FET gate load (p65 rule): worst case 1.69 uC droops 11 V by 1.7 V, pump start about 130 ms.
3. OTP owed: DFETOFF Pin Config 0x42 (active high, BOTHOFF), CFETOFF and HDQ as thermistor inputs. Presence and DFETOFF logic are on HP_power_board.
   Cell 1 Interconnect stays 0: VC0 reads the Kelvin B0 tab. VC0 sits below VSS by the 10 AWG return drop, about 90 mV at 60 A (abs max VSS - 0.3 V).
4. RS1 land: CSS4J-4026 on the BVR4026 library footprint, check against the Bourns drawing.'''
P.text(NOTE1,(20.32,215.9),size=1.5)
P.text(NOTE2,(20.32,233.68),size=1.5)
_a,_b=(40.64,160.02),(55.88,160.02)
P.sym('Device:R',Rr(),'10k',(48.26,160.02),90,R603,refat=(48.26,157.48),valat=(48.26,162.56))
P.wire((44.45,160.02),_a); P.label('B0',_a,180); P.wire((52.07,160.02),_b); P.label('BAT-',_b)
P.text('Keeps VC0 defined with the harness out. 9 uA at 60 A. D15 clamps VC0 near -0.3 V in a short, no guaranteed margin. Keep the B0 to VSS return under 1 mOhm in layout: -0.6 V at 600 A, about 3 mA through R14.',(30.48,165.1))
P.wire((68.58,160.02),(73.66,160.02)); P.label('VC0',(73.66,160.02)); P.sym('Device:D','D15','B140-13-F 40V 1A',(64.77,160.02),180,'Diode_SMD:D_SMA',refat=(64.77,157.48),valat=(64.77,162.56)); vss((60.96,160.02),270)
# ---------- hierarchy ----------
def ph(key,title,extra=None):
    s=Sheet(key,title,'A4',comment='Placeholder, not yet captured'); s.file=key+'.kicad_sch'
    s.text('Placeholder. '+(extra or 'Not yet captured.'),(25.4,35.56),size=2)
    return s
HP=Sheet('HP_power_board','HP_power_board: high-power board','A3'); HP.file='HP_power_board.kicad_sch'
LP=Sheet('LP_logic_board','LP_logic_board: low-power board','A3'); LP.file='LP_logic_board.kicad_sch'
R=Sheet('root','marengo power board rev A','A3'); R.file='marengo_pdu.kicad_sch'
page=[1]
def pg():
    page[0]+=1; return str(page[0])
writes=[]
hid=U('sheet/HP'); lid=U('sheet/LP'); stub=U('sheet/pi'); pid=U('sheet/packsense')
rp='/'+ROOT
sheetsym(R,'HP_power_board','HP_power_board.kicad_sch',(50.8,50.8),(76.2,50.8),[],(rp,'2'),hid)
page[0]=2
writes.append((HP,[(f'{rp}/{hid}','2')]))
pnames=[('VSYS','passive'),('PACK_TOP','passive'),('CHG_GATE','output'),('DSG_GATE','output'),('PDSG','output'),('I2C_SDA','bidirectional'),('I2C_SCL','bidirectional'),('ALERT','output'),('DFETOFF','input'),('RST_SHUT','input'),('NTC1','input'),('NTC2','input'),('NTC3','input'),('BOARD_NTC','input'),('PACK_PRESENT_N','passive'),('TS2_WAKE','input')]
ppin=U('sheet/pack_input_shunt_afe'); pgn=pg()
ends=sheetsym(HP,'pack_input_shunt_afe',P.file,(38.1,38.1),(50.8,48.26),pnames,(f'{rp}/{hid}',pgn),ppin)
LINKED={'VSYS','PACK_TOP','CHG_GATE','DSG_GATE','PDSG'}
writes.append((P,[(f'{rp}/{hid}/{ppin}',pgn)]))
import sheets
sheets.G=globals()
import sheets2 as S2
pwr263s=S2.pwr263s; v10pm15=S2.v10pm15; BAR=S2.BAR
S2.reg()
B=sheets.bms(); sid=U('sheet/bms_fets_precharge'); pn=pg()
ee=sheetsym(HP,'bms_fets_precharge',B.file,(38.1,109.22),(50.8,27.94),sheets.BMS_PINS,(f'{rp}/{hid}',pn),sid); writes.append((B,[(f'{rp}/{hid}/{sid}',pn)]))
P.paper='A2'; S2.balance(P)
HP.refc={p:100 for p in ('R','C','D','Q','U','J','L','F','TP','RS','RT','#PWR','#FLG')}; LP.refc={p:2300 for p in ('R','C','D','Q','U','J','L','F','TP','RS','RT','#PWR','#FLG')}
LINK={'VSYS':'VSYS','PACK_TOP':'PACK_TOP','CHG_GATE':'CHG_GATE','DSG_GATE':'DSG_GATE','PDSG':'PDSG','I2C_SDA':'I2C_AFE_SDA','I2C_SCL':'I2C_AFE_SCL','ALERT':'AFE_ALERT','DFETOFF':'DFETOFF','RST_SHUT':'AFE_RST_SHUT','BOARD_NTC':'BOARD_NTC','PACK_PRESENT_N':'PACK_PRESENT_N','TS2_WAKE':'TS2_WAKE','CHARGER_PRESENT':'CHARGER_PRESENT','I2C_HP_SDA':'I2C_HP_SDA','I2C_HP_SCL':'I2C_HP_SCL','+3V3_HP':'+3V3_HP'}
def link(pins,ends):
    for (nm,_),e in zip(pins,ends):
        t=LINK.get(nm)
        if not t: HP.nc(e); continue
        HP.wire(e,(e[0]+2.54,e[1])); (HP.glabel if t in sheets.GLOBALS else HP.label)(t,(e[0]+2.54,e[1]))
HP.paper='A2'
link(pnames,ends); link(sheets.BMS_PINS,ee)
for k,(nm,fn) in enumerate([('estop_banks',S2.estop),('vmot_bulk_clamp',S2.vmot)]):
    s=fn(); sid=U('sheet/'+nm); pn=pg()
    sheetsym(HP,nm,s.file,(152.4+k*66.04,109.22),(50.8,20.32),[],(f'{rp}/{hid}',pn),sid); writes.append((s,[(f'{rp}/{hid}/{sid}',pn)]))
pc=S2.port(); insts=[]
STRAP=[('GND','GND'),('GND','+3V3_HP'),('+3V3_HP','GND'),('GND','I2C_HP_SCL'),('+3V3_HP','I2C_HP_SCL'),('+3V3_HP','+3V3_HP')]
NM=['L_ARM','R_ARM','WAIST','L_LEG','R_LEG','AUX']
for k in range(6):
    sid=U(f'sheet/port{k}'); pn=pg(); K=k+1
    e=sheetsym(HP,f'port_{K}_{NM[k]}',pc.file,(38.1+k*66.04,157.48),(40.64,25.4),[(p,'passive') for p in S2.PORT_PINS],(f'{rp}/{hid}',pn),sid,dnp=K in(4,5)); insts.append((f'{rp}/{hid}/{sid}',pn))
    tgt=dict(zip(S2.PORT_PINS,[f'PORT_EN{K}',f'IMON{K}',f'CAN{K}_H',f'CAN{K}_L',STRAP[k][0],STRAP[k][1],f'V_PORT{K}']))
    for p,q in zip(S2.PORT_PINS,e):
        if p=='V_PORT' and K!=6: HP.nc(q); continue
        b=(q[0]+2.54,q[1]); HP.wire(q,b); sheets.put(HP,b,tgt[p],'R')
writes.append((pc,insts))
S2.hp_extras(HP)
sheetsym(R,'LP_logic_board','LP_logic_board.kicad_sch',(152.4,50.8),(76.2,50.8),[],(rp,pg()),lid); lpn=str(page[0]); writes.append((LP,[(f'{rp}/{lid}',lpn)]))
LP.paper='A2'
for k,(nm,fn) in enumerate([('supervisor_mcu',S2.supervisor),('can_bridges_usb_hub',S2.bridges)]):
    s=fn(); sid=U('sheet/'+nm); pn=pg(); sheetsym(LP,nm,s.file,(38.1+k*63.5,38.1),(50.8,20.32),[],(f'{rp}/{lid}',pn),sid); writes.append((s,[(f'{rp}/{lid}/{sid}',pn)]))
ic=S2.isocan(); insts=[]
for k in range(6):
    sid=U(f'sheet/iso{k}'); pn=pg(); K=k+1
    e=sheetsym(LP,f'can_phy_{K}',ic.file,(38.1+k*66.04,76.2),(40.64,17.78),[(p,'passive') for p in S2.ISO_PINS],(f'{rp}/{lid}',pn),sid); insts.append((f'{rp}/{lid}/{sid}',pn))
    tgt=dict(zip(S2.ISO_PINS,[f'CAN{K}_TXD',f'CAN{K}_RXD',f'CAN{K}_H',f'CAN{K}_L']))
    for p,q in zip(S2.ISO_PINS,e):
        b=(q[0]+2.54,q[1]); LP.wire(q,b); sheets.put(LP,b,tgt[p],'R')
writes.append((ic,insts))
for k,(nm,fn) in enumerate([('front_end_12v_24v',sheets.front_end),('pol_rails',sheets.pol),('rail_switches_meters',sheets.rails),('io_fans_estop_loop',S2.io)]):
    s=fn(); sid=U('sheet/'+nm); pn=pg(); sheetsym(LP,nm,s.file,(38.1+k*63.5,124.46),(50.8,20.32),[],(f'{rp}/{lid}',pn),sid); writes.append((s,[(f'{rp}/{lid}/{sid}',pn)]))
S2.lp_extras(LP)
for nm,at,sid,fn in (('pi_power_stub',(50.8,127),stub,S2.pistub),('pack_sense_board',(152.4,127),pid,S2.packsense)):
    s=fn(); pn=pg(); sheetsym(R,nm,s.file,at,(76.2,25.4),[],(rp,pn),sid); writes.append((s,[(f'{rp}/{sid}',pn)]))
writelib()
R.write([(rp,'1')])
for s,i in writes: s.write(i)
print('pages',page[0])
