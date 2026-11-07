"""Schematic kit for rev B, cut down from the rev A generator: symbols, one flat sheet, label-style blocks."""
import sys,uuid,math,copy,re,os,json
sys.path.insert(0,os.path.join(os.path.dirname(os.path.abspath(__file__)),'..','tools'))
from sx import *
LIB=next((p for p in (os.environ.get('KICAD_SYM'),'/Applications/KiCad/KiCad.app/Contents/SharedSupport/symbols/','C:/Program Files/KiCad/10.0/share/kicad/symbols/') if p and os.path.isdir(p)),None)
assert LIB,'set KICAD_SYM to the KiCad symbols folder'
def wfile(path): return open(path,'w',encoding='utf-8',newline=chr(10))
OUT=sys.argv[1]
PROJ='marengo_pdu_revb'; LIBNAME='marengo_revb'; NS=uuid.UUID('b1d0f6a2-58a7-4a55-9e8e-2d4b6c7e1f02')
ROOT=str(uuid.uuid5(NS,'root'))
PARTS=os.path.join(os.path.dirname(os.path.abspath(__file__)),'parts')
def U(s): return str(uuid.uuid5(NS,s))
FONT=[ 'font',['size','1.27','1.27']]
n=lambda v:('%.4f'%v).rstrip('0').rstrip('.')
def eff(j=None,hide=False,bold=False,size=1.27):
    e=['effects',['font',['size',n(size),n(size)]]+([['bold','yes']] if bold else [])]
    if j: e.append(['justify']+j.split())
    if hide: e.append(['hide','yes'])
    return e
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
    wfile(OUT+'/'+LIBNAME+'.kicad_sym').write(dump(['kicad_symbol_lib',['version','20251024'],['generator',S('kicad_symbol_editor')],['generator_version',S('10.0')]]+[CUSTOM[k] for k in sorted(CUSTOM)])+'\n')
# ---------- library access ----------
_c={}
def libsym(lid):
    lib,name=lid.split(':')
    if lib==LIBNAME: s=copy.deepcopy(CUSTOM[name])
    else:
        if lib not in _c: _c[lib]={s[1]:s for s in getall(parse(open(LIB+lib+'.kicad_sym',encoding='utf-8').read()),'symbol')}
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
        tb=['title_block',['title',S(s.title)],['date',S('2026-11-06')],['rev',S('B-wip')],['company',S('marengo')]]
        if s.comment: tb.append(['comment','1',S(s.comment)])
        doc=['kicad_sch',['version','20260306'],['generator',S('eeschema')],['generator_version',S('10.0')],['uuid',S(ROOT if s.key=='root' else U('file/'+s.key))],['paper',S(s.paper)],tb,['lib_symbols']+list(s.lib.values())]+items
        if s.key=='root': doc.append(['sheet_instances',['path',S('/'),['page',S('1')]]])
        doc.append(['embedded_fonts','no'])
        wfile(f'{OUT}/{s.file}').write(dump(doc)+'\n')
# rev B5 hand assembly (Joey 17:26Z): no 0402 anywhere; the small default is 0603
R402='Resistor_SMD:R_0603_1608Metric';C402='Capacitor_SMD:C_0603_1608Metric';C805='Capacitor_SMD:C_0805_2012Metric';C1210='Capacitor_SMD:C_1210_3225Metric'
R603='Resistor_SMD:R_0603_1608Metric';C603='Capacitor_SMD:C_0603_1608Metric';C1206='Capacitor_SMD:C_1206_3216Metric';R1206='Resistor_SMD:R_1206_3216Metric'
SOD='Diode_SMD:D_SOD-123';SOT23='Package_TO_SOT_SMD:SOT-23';SMC='Diode_SMD:D_SMC';MOS5='Package_TO_SOT_SMD:TDSON-8-1'
def gnd(s,p): s.sym('power:GND',s.nref('#PWR'),'GND',p,0,hideref=True)
def gsym(s,p,d):
    s.sym('power:GND',s.nref('#PWR'),'GND',p,{'L':270,'R':90,'U':180}[d],hideref=True)
def put(s,p,net,d):
    """end a stub at p with net (ground symbol, global label or local label)"""
    a={'R':0,'L':180,'U':90,'D':270}[d]
    if net=='GND': gnd(s,p) if d=='D' else gsym(s,p,d)
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
        val,fp=cnorm(val,fp)
        for _ in range(k): g.two('Device:C','C',val,a,b,fp)
    def CP(g,val,a,b,fp): return g.two('Device:C_Polarized','C',val,a,b,fp)
    def L(g,val,a,b,fp): return g.two('Device:L','L',val,a,b,fp)
    def D(g,val,k,an,fp=SOD): return g.two('Device:D','D',val,k,an,fp,rot=270)
    def Z(g,val,k,an,fp=SOD): return g.two('Device:D_Zener','D',val,k,an,fp,rot=270)
# rev B5 BOM consolidation (hand assembly, one kit): one part per value/size. A higher rating replaces a lower one only
# where the bias derating still meets the circuit (X7R 50 V 0603 100 nF keeps about 90 % at 12 V).
CNORM={('100nF','C_0603'):('100nF 50V X7R',None),('100nF 25V','C_0603'):('100nF 50V X7R',None),('100nF 25V X7R','C_0603'):('100nF 50V X7R',None),
       ('100nF 100V X7R','C_0603'):('100nF 100V X7R','Capacitor_SMD:C_0805_2012Metric'),('100nF 100V','C_0805'):('100nF 100V X7R',None),
       ('1uF 100V','C_1210'):('1uF 100V X7R',None),('2.2uF 100V','C_1210'):('2.2uF 100V X7R',None),('22uF 25V','C_1210'):('22uF 25V X7R',None),
       ('2.2uF 25V','C_0805'):('2.2uF 25V X7R',None)}
def cnorm(val,fp):
    k=(val,fp.split(':')[-1][:6]); n=CNORM.get(k)
    return (n[0],n[1] or fp) if n else (val,fp)
def ext(l):
    ys=[]
    for u in getall(l,'symbol'):
        for p in getall(u,'pin'):
            ys.append(float(get(p,'at')[2]))
    return (min(ys),max(ys)) if ys else (-10,10)
def nic(s,lid,ref,val,at,fp,nets,ds='',rot=0,L=5.08):
    at=(round(at[0]/1.27)*1.27,round(at[1]/1.27)*1.27)
    l=libsym(lid); lo,hi=ext(l)
    s.sym(lid,ref,val,at,rot,fp,ds=ds,refat=(at[0]-5.08,at[1]-hi-(11.43)),valat=(at[0]-5.08,at[1]-lo+(11.43)))
    seen={}
    for u in getall(l,'symbol'):
        for p in getall(u,'pin'):
            num=str(get(p,'number')[1]); a=get(p,'at'); ang=(int(float(a[3]))+rot)%360
            q=pos(l,num,at,rot); d={0:'L',180:'R',90:'D',270:'U'}[ang]
            net=nets.get(num,'__miss__'); assert net!='__miss__',(ref,num)
            if q in seen: assert seen[q]==net,(ref,num); continue
            seen[q]=net
            if net is None: s.nc(q); continue
            dx,dy={'R':(L,0),'L':(-L,0),'U':(0,-L),'D':(0,L)}[d]; e=(q[0]+dx,q[1]+dy)
            s.wire(q,e); put(s,e,net,d)
def custom(name,jf,fp=None,ds=None,ren=None,et=None,renum=None):
    d=json.load(open(os.path.join(PARTS,jf+'.json'),encoding='utf-8'))
    pins=[]
    for p in d['pins']:
        nm=str(p['name']).split(' ')[0].split('(')[0]; nm=(ren or {}).get(str(p['num']),nm)
        pins.append(((renum or {}).get(str(p['num']),str(p['num'])),nm,(et or {}).get(str(p['num']),p['etype'])))
    names=[p[1] for p in pins]; pins=[(a,b,'passive' if c=='power_out' and names.count(b)>1 else c) for a,b,c in pins]
    if name not in CUSTOM: mkic(name,pins,fp if fp is not None else (d.get('kicad_footprint') or ''),(d.get('datasheet') or {}).get('url',''),d.get('part',''))
    return LIBNAME+':'+name
def nfet5(s,ref,val,at,d,g,so,lid='Transistor_FET:Q_NMOS_SSSGD_AvalancheRated',fp=MOS5):
    """N-FET by pin name"""
    l=libsym(lid); nets={}
    for u in getall(l,'symbol'):
        for p in getall(u,'pin'):
            nm=str(get(p,'name')[1]); nets[str(get(p,'number')[1])]={'S':so,'G':g,'D':d}[nm[0]]
    nic(s,lid,ref,val,at,fp,nets)
