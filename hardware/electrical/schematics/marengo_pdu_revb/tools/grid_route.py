#!/usr/bin/env python3
"""Stage 3 (own router), step 2: route every unconnected net on a 0.1 mm grid. Plain Python (numpy, scipy, PIL).
Freerouting thrashed on this board (Freerouting notes in LAYOUT_RULES_REVB.md), so this is a simple, predictable
sequential maze router:
  - copper from tools/route_export.py is rasterized per layer (owner = net); the In1 GND fill only joins GND for
    connectivity; In1 is never routed, so it stays a plane;
  - each net's copper is split into connected islands (per layer, joined through its vias and plated holes);
  - islands are joined one at a time, shortest nets first, by A* over F.Cu, In2 and B.Cu (8-way moves, a via costs 30
    steps), inside a window round the two islands that grows on failure;
  - a cell is usable when its distance to other nets' copper keeps the rules: track half-width + 0.2 mm, or + 0.6 mm to
    54 V copper on the outer layers (IPC-2221B), + 0.2 mm to holes, + 0.4 mm to the board edge. A via (0.6/0.3) needs the
    same on all four layers;
  - classes: signals 0.2 mm, GND and 54 V nets 0.25 mm (sense and divider taps off 0.5 mm-pitch pins; the current is in the pours),
    12 V distribution 0.6 mm (W12).
Output: routes.json (tracks as polylines per layer with width, vias) for tools/route_import.py, plus a list of what failed.
Usage: python tools/grid_route.py <copper.json> <routes.json>"""
import os,heapq,json,math,re,sys,time
import numpy as np
from PIL import Image,ImageDraw
from scipy import ndimage
RES=0.1
D=json.load(open(sys.argv[1]))
X0,Y0,X1,Y1=D['edge']; W=int(math.ceil((X1-X0)/RES))+1; H=int(math.ceil((Y1-Y0)/RES))+1
NETS=sorted({it['net'] for it in D['items'] if it['net']}|{h['net'] for h in D['holes'] if h['net']})
NID={n:i+1 for i,n in enumerate(NETS)}
sys.path.insert(0,os.path.dirname(os.path.abspath(__file__))); import domains   # the net classes route_checks.py tests
P12={'/+12V_RAW','/+12V','/+12V_JET','/+12V_PI','/SPARE_SW','/SPARE_ON','/SP_SW1'}
HV=np.zeros(len(NETS)+1,bool); ES=np.zeros(len(NETS)+1,bool)
for n,i in NID.items(): HV[i]=domains.is_hv(n); ES[i]=domains.is_estop(n)
# voltage domains (tools/domains.py): 0 = ground-referenced; copper riding on one 54 V node shares a domain; a 54 V net
# outside every domain is a domain of its own. 0.6 mm (outer layers) between different domains where one isn't 0,
# 0.2 mm otherwise; E-stop nets keep 1.5 mm to 54 V nets on every layer. Close to the routed net's own pads (escape
# zone) the 54 V clearance relaxes to 0.2 mm (ESC) and the E-stop one to 0.2 mm (ESCE): fine-pitch ICs put pins of
# different domains 0.5 mm apart, which the package sets, not the routing.
DOMN={None:0}; DOM=np.zeros(len(NETS)+1,np.int32)
for n,i in NID.items():
    k=domains.dom(n) or (n if HV[i] else None); DOM[i]=DOMN.setdefault(k,len(DOMN))
def width(n): return W12 if n in P12 else 0.25 if (n=='GND' or HV[NID[n]]) else 0.2   # 54 V taps carry no current: the pours do
def g(x,y): return ((x-X0)/RES,(y-Y0)/RES)
# ---- rasterize
KEEP=-3; NONET=-4   # keep-outs; copper with no net (mounting-hole pads, connector pegs)
M=0.10   # rasterization margin on every clearance (0.1 mm grid)
ESC=float(__import__("os").environ.get("ESC","0.8"))   # escape zone round the net's own pads where 54 V clearance relaxes to 0.2 mm
ESCE=1.0   # the same for the E-stop 1.5 mm rule
HVCL=float(os.environ.get("HVCL","0.2" if domains.COATED else "0.6"))   # outer-layer clearance between voltage domains: 0.6 uncoated (IPC-2221B B2), 0.2 coated (B4: 0.13)
EM=float(__import__("os").environ.get("EM","-0.06"))   # clearance margin inside the escape zone (cancels rasterization growth)
HW=float(os.environ.get('HW','1.5'))   # A* heuristic weight
NEG=os.environ.get('NEG','1')=='1'   # negotiated congestion (PathFinder) instead of sequential rip-up
NITER=int(os.environ.get('NITER','25')); PRES=1.0
NECK=0.25   # track width within the escape zone (all classes)
W12=float(__import__("os").environ.get("W12","0.6"))   # 12 V distribution track width (2 oz outer: about 4 A at 20 K rise; the 12 A trunk is the +12V_RAW strip)
own=[np.zeros((H,W),np.int32) for _ in range(4)]
padown=[np.zeros((H,W),np.int32) for _ in range(4)]   # pads only, for the escape zones
gndplane=np.zeros((H,W),bool)
def fill(img_arr,poly,val):
    im=Image.new('I',(W,H),0); dr=ImageDraw.Draw(im); hole=[]
    for ring in poly:
        if ring and ring[-1] is None: hole.append(ring[:-1]); continue
        dr.polygon([g(*p) for p in ring],fill=1)
    for ring in hole: dr.polygon([g(*p) for p in ring],fill=0)
    m=np.array(im,dtype=np.int32)>0
    img_arr[m]=val
    return m
# faster: one image per (layer, net) batch
from collections import defaultdict
batches=defaultdict(list)
for it in D['items']:
    v=KEEP if it['kind']=='keepout' else NID.get(it['net'],NONET)
    if it['kind']=='plane': batches[('plane',)].append(it['poly']); continue
    batches[(it['layer'],v,it['kind']=='pad')].append(it['poly'])
for key,pl in batches.items():
    im=Image.new('L',(W,H),0); dr=ImageDraw.Draw(im)
    for poly in pl:
        holes=[]
        for ring in poly:
            if ring and ring[-1] is None: holes.append(ring[:-1]); continue
            if len(ring)>2: dr.polygon([g(*p) for p in ring],fill=1)
        for ring in holes:
            if len(ring)>2: dr.polygon([g(*p) for p in ring],fill=0)
    m=np.array(im)>0
    if key[0]=='plane': gndplane|=m
    else:
        own[key[0]][m]=key[1]
        if key[2]: padown[key[0]][m]=key[1]
# exact pad distances: the 0.1 mm raster puts a pad edge up to ~0.07 mm off, which at 0.5 mm pin pitch is the whole
# margin. Per layer: D1/N1 = distance (mm, cell centre to pad outline) and net of the nearest pad, D2 = distance to the
# nearest pad of any other net; the foreign-pad distance for net v is D2 where N1 == v, else D1.
FINE={'U1','U2','U7'}   # LM5069 MSOP-10 and TPS7A4001 HVSSOP-8: pads 0.15 mm apart, so 0.15 mm to their pads inside their courtyard only (layout/*.kicad_dru)
PM=0.03   # margin on exact pad clearances (polyline vertices sit on cell centres, so the geometry is exact)
D1=[np.full((H,W),99.0) for _ in range(4)]; D2=[np.full((H,W),99.0) for _ in range(4)]; N1=[np.zeros((H,W),np.int32) for _ in range(4)]
padctr=[np.zeros((H,W),bool) for _ in range(4)]
def _pt_poly(px_,py_,ring):
    a=np.array(ring,float); b=np.roll(a,-1,axis=0); d=np.full(px_.shape,1e9)
    for (ax,ay),(bx,by) in zip(a,b):
        ex,ey=bx-ax,by-ay; L2=ex*ex+ey*ey or 1e-12
        t=np.clip(((px_-ax)*ex+(py_-ay)*ey)/L2,0,1); d=np.minimum(d,np.hypot(px_-ax-t*ex,py_-ay-t*ey))
    # inside test (even-odd)
    ins=np.zeros(px_.shape,bool)
    for (ax,ay),(bx,by) in zip(a,b):
        c=((ay>py_)!=(by>py_))&(px_<(bx-ax)*(py_-ay)/((by-ay) if by!=ay else 1e-12)+ax); ins^=c
    return np.where(ins,0.0,d)
# courtyard boxes of the FINE footprints (pad extents + 0.25 mm, cells): relaxed pad distances apply only inside
fbox={}
for it in D['items']:
    r=it.get('ref','').split('.')[0]
    if it['kind']=='pad' and r in FINE:
        for ring in it['poly']:
            if ring and ring[-1] is not None:
                b=fbox.setdefault(r,[1e9,1e9,-1e9,-1e9])
                for x,y in ring: b[0]=min(b[0],x); b[1]=min(b[1],y); b[2]=max(b[2],x); b[3]=max(b[3],y)
incy=np.zeros((H,W),bool)
for x0_,y0_,x1_,y1_ in fbox.values():
    incy[max(0,int((y0_-0.25-Y0)/RES)):int((y1_+0.25-Y0)/RES)+1,max(0,int((x0_-0.25-X0)/RES)):int((x1_+0.25-X0)/RES)+1]=True
for it in D['items']:
    if it['kind']!='pad': continue
    L=it['layer']; pv=NID.get(it['net'],NONET)
    relax=0.05 if it.get('ref','').split('.')[0] in FINE else 0.0
    for ring in it['poly']:
        if not ring or ring[-1] is None or len(ring)<3: continue
        xs=[q[0] for q in ring]; ys=[q[1] for q in ring]
        c0_=max(0,int((min(xs)-1.2-X0)/RES)); c1_=min(W,int((max(xs)+1.2-X0)/RES)+2)
        r0_=max(0,int((min(ys)-1.2-Y0)/RES)); r1_=min(H,int((max(ys)+1.2-Y0)/RES)+2)
        if r1_<=r0_ or c1_<=c0_: continue
        gy,gx=np.mgrid[r0_:r1_,c0_:c1_]; d=_pt_poly(X0+gx*RES,Y0+gy*RES,ring)
        if relax: d=np.where(incy[r0_:r1_,c0_:c1_],np.maximum(d-relax,0),d)
        d1=D1[L][r0_:r1_,c0_:c1_]; d2=D2[L][r0_:r1_,c0_:c1_]; n1=N1[L][r0_:r1_,c0_:c1_]
        closer=d<d1; other=n1!=pv
        d2[closer&other]=d1[closer&other]
        d2[~closer&other&(d<d2)]=d[~closer&other&(d<d2)]
        d1[closer]=d[closer]; n1[closer]=pv
        cx_,cy_=g(sum(xs)/len(xs),sum(ys)/len(ys)); cy_,cx_=int(round(cy_)),int(round(cx_))
        if 0<=cy_<H and 0<=cx_<W: padctr[L][cy_,cx_]=True
# escape stubs: on a fine-pitch pad (narrow side < 0.45 mm) a track clears the neighbouring pins only on the pad's own
# axis, which falls up to 0.05 mm off the grid. The grid row/column nearest the axis is usable for that net through the
# pad and 0.6 mm past each end, where the exact distance from the true axis to every other pad keeps the clearance;
# exported points on a stub are snapped onto the true axis (stamp()).
STUB=[np.zeros((H,W),np.int32) for _ in range(4)]; SNAPX=[np.full((H,W),np.nan) for _ in range(4)]; SNAPY=[np.full((H,W),np.nan) for _ in range(4)]
_pads=[]
for it in D['items']:
    if it['kind']!='pad': continue
    for ring in it['poly']:
        if ring and ring[-1] is not None and len(ring)>2:
            xs=[q[0] for q in ring]; ys=[q[1] for q in ring]
            _pads.append((it['layer'],NID.get(it['net'],NONET),it.get('ref','').split('.')[0],ring,min(xs),min(ys),max(xs),max(ys)))
for L,pv,ref,ring,x0p,y0p,x1p,y1p in _pads:
    w_,h_=x1p-x0p,y1p-y0p
    if pv<=0 or min(w_,h_)>=0.45 or max(w_,h_)<1.8*min(w_,h_): continue
    cx,cy=(x0p+x1p)/2,(y0p+y1p)/2; hwn=min(width(NETS[pv-1]),NECK)/2
    if h_>w_: pts=[(cx,y) for y in np.arange(y0p-0.6,y1p+0.6+1e-9,RES)]
    else: pts=[(x,cy) for x in np.arange(x0p-0.6,x1p+0.6+1e-9,RES)]
    near=[q for q in _pads if q[0]==L and q[1]!=pv and q[4]<x1p+1.5 and q[6]>x0p-1.5 and q[5]<y1p+1.5 and q[7]>y0p-1.5]
    for x,y in pts:
        c,r=int(round((x-X0)/RES)),int(round((y-Y0)/RES))
        if not (0<=r<H and 0<=c<W): continue
        ok_=True
        for _,qv,qref,qring,*_b in near:
            cl=0.15 if (qref in FINE and incy[r,c]) else 0.2
            if _pt_poly(np.array([x]),np.array([y]),qring)[0]<hwn+cl+0.01: ok_=False; break
        if not ok_: continue
        STUB[L][r,c]=pv
        if h_>w_: SNAPX[L][r,c]=cx
        else: SNAPY[L][r,c]=cy
hole=np.zeros((H,W),np.int32); holecells={}
im=Image.new('I',(W,H),0); dr=ImageDraw.Draw(im)
for h in D['holes']:
    v=NID.get(h['net'],-2)
    if 'circle' in h:
        x,y,r=h['circle']; cx,cy=g(x,y); rr=r/RES; dr.ellipse([cx-rr,cy-rr,cx+rr,cy+rr],fill=v)
        if v>0: holecells.setdefault(v,[]).append((int(round(cy)),int(round(cx))))
    else:
        for ring in h['poly']:
            if ring and ring[-1] is not None and len(ring)>2:
                dr.polygon([g(*p) for p in ring],fill=v)
                if v>0:
                    xs=[p[0] for p in ring]; ys=[p[1] for p in ring]; cx,cy=g(sum(xs)/len(xs),sum(ys)/len(ys)); holecells.setdefault(v,[]).append((int(round(cy)),int(round(cx))))
hole=np.array(im,dtype=np.int32)
inside=np.zeros((H,W),bool); inside[int(0.4/RES):H-int(0.4/RES),int(0.4/RES):W-int(0.4/RES)]=True
# routed copper lives in its own layers so it can be ripped up: dyn (tracks and vias), dynhole (via holes)
dyn=[np.zeros((H,W),np.int32) for _ in range(4)]; dynhole=np.zeros((H,W),np.int32)
ROUTES={}   # net id -> list of committed connections: dict(cells=[(L,ys,xs)], vias=[(Y,X)], segs, vias_mm)
print(f'grid {W}x{H}, nets {len(NETS)}',flush=True)
# ---- connectivity
IN1=os.environ.get('IN1','1')=='1'
RL=[0,1,2,3] if IN1 else [0,2,3]   # routable layers: F.Cu, (In1), In2, B.Cu
# In1 is solid GND; LAYOUT_RULES_REVB.md keeps it unbroken under the banks, the absorber, the bus caps and the port FETs.
# Signals may use it, at twice the cost, only in the control areas (floorplan rects X0, X1, Y0, Y1): the bottom
# 12 V band, the latch / soft start / bank B controller column, the out block's middle and the bank A / clamp column
IN1RECTS=[(-49.4,49.4,171.0,200.5),(-49.4,-18.5,266.0,329.0),(-49.4,-29.0,254.0,266.0),(-6.0,6.0,201.0,253.0),
          (18.5,49.4,266.0,305.0),(29.0,49.4,254.0,266.0),
          (9.5,19.5,201.0,253.5),(-19.5,-9.5,201.0,253.5)]   # the port blocks inboard of the port FETs (X 20-32); the outboard side is the return corridor
IN1OK=np.zeros((H,W),bool)
for fx0,fx1,fy0,fy1 in IN1RECTS:   # floorplan (X, Y up) -> board (x = X + 150, y = 440 - Y)
    c0_,c1_=int((fx0+150-X0)/RES),int((fx1+150-X0)/RES); r0_,r1_=int((440-fy1-Y0)/RES),int((440-fy0-Y0)/RES)
    IN1OK[max(0,r0_):min(H,r1_),max(0,c0_):min(W,c1_)]=True
def islands(v,box=None):
    """connected copper islands of net v: list of sets of (layer, flat index) and a label map per layer"""
    labs=[]; nl=0; parent={}
    for L in range(4):
        m=(own[L]==v)|(dyn[L]==v)
        if L==1 and NETS[v-1]=='GND': m=m|gndplane
        lab,n=ndimage.label(m,structure=np.ones((3,3)))
        lab=np.where(lab>0,lab+nl,0); labs.append(lab); nl+=n
    for i in range(1,nl+1): parent[i]=i
    def f(a):
        while parent[a]!=a: parent[a]=parent[parent[a]]; a=parent[a]
        return a
    for (r,c) in holecells.get(v,[])+[q for rec in ROUTES.get(v,[]) for q in rec['vias']]:
        if not (0<=r<H and 0<=c<W): continue
        ids=[labs[L][r,c] for L in range(4) if labs[L][r,c]]
        for a in ids[1:]: parent[f(a)]=f(ids[0])
    comp={}; rootmap=np.array([0]+[f(i) for i in range(1,nl+1)])
    for L in range(4):
        lab=labs[L]; ys,xs=np.nonzero(lab)
        if not len(ys): continue
        roots=rootmap[lab[ys,xs]]
        for rt in np.unique(roots):
            sel=roots==rt; comp.setdefault(int(rt),[]).append((L,ys[sel],xs[sel]))
    return list(comp.values())
# ---- per-window cost fields
def fields(v,r0,r1,c0,c1,base_only=False):
    """boolean usable masks for a track of net v on each routable layer and for a via, in the window"""
    hw=width(NETS[v-1])/2; hv=HV[v]
    ok={}; ESCM={}
    # base_only: False = all routed copper is an obstacle; otherwise an array of locked net ids whose routed copper still
    # counts (the rest is treated as rippable and ignored)
    keep=(lambda a: a) if base_only is False else (lambda a: np.where(np.isin(a,base_only),a,0))
    hl=hole[r0:r1,c0:c1]
    dh_=keep(dynhole[r0:r1,c0:c1]); hl=np.where(dh_!=0,dh_,hl)
    foreign_hole=(hl!=0)&(hl!=v)
    dh=ndimage.distance_transform_edt(~foreign_hole)*RES if foreign_hole.any() else np.full(hl.shape,99.0)
    edge=inside[r0:r1,c0:c1]
    de=ndimage.distance_transform_edt(edge)*RES
    via_ok=(dh>=0.3+0.25+M)&(de>=0.3+0.4)
    for L in range(4):
        pd=padown[L][r0:r1,c0:c1]
        o=own[L][r0:r1,c0:c1]
        d_=keep(dyn[L][r0:r1,c0:c1]); o=np.where(d_!=0,d_,o)
        onp=np.where((pd!=0)&(d_==0),0,o)   # copper without the pads: pads are checked exactly (px) below
        fo=(onp!=0)&(onp!=v)
        od=DOM[np.clip(onp,0,None)]; dv_=DOM[v]
        fhv=fo&(((onp>0)&(od!=dv_)&((od!=0)|(dv_!=0)))|((onp==NONET)&(dv_!=0)))   # different voltage domain, one of them 54 V (no-net copper counts as ground)
        flv=fo&~fhv
        fes=fo&(onp>0)&(ES[np.clip(onp,0,None)] if HV[v] else HV[np.clip(onp,0,None)]) if (HV[v] or ES[v]) else np.zeros(o.shape,bool)   # E-stop vs 54 V
        fesp=(pd>0)&(pd!=v)&(ES[np.clip(pd,0,None)] if HV[v] else HV[np.clip(pd,0,None)]) if (HV[v] or ES[v]) else np.zeros(o.shape,bool)
        des=ndimage.distance_transform_edt(~(fes|fesp))*RES if (fes|fesp).any() else np.full(o.shape,99.0)
        esce=ndimage.distance_transform_edt(pd!=v)*RES<=ESCE if (pd==v).any() else np.zeros(o.shape,bool)
        es_ok=esce|(des>=hw+1.5+M)
        es_vok=esce|(des>=0.3+1.5+M)
        pdom=DOM[np.clip(pd,0,None)]
        fhvp=(((pd>0)&(pd!=v)&(pdom!=dv_)&((pdom!=0)|(dv_!=0)))|((pd==NONET)&(dv_!=0)))   # 54 V pads of another domain (raster, used outside the escape zone)
        esc=ndimage.distance_transform_edt(pd!=v)*RES<=ESC if (pd==v).any() else np.zeros(o.shape,bool)
        dl=ndimage.distance_transform_edt(~flv)*RES if flv.any() else np.full(o.shape,99.0)
        dv=ndimage.distance_transform_edt(~fhv)*RES if fhv.any() else np.full(o.shape,99.0)
        dvp=ndimage.distance_transform_edt(~fhvp)*RES if fhvp.any() else np.full(o.shape,99.0)
        n1=N1[L][r0:r1,c0:c1]; px=np.where(n1==v,D2[L][r0:r1,c0:c1],D1[L][r0:r1,c0:c1])   # exact foreign-pad distance
        outer=L in (0,3)
        clhv=np.where(esc,0.2,HVCL if outer else 0.2); cllv=0.2
        hvp_ok=esc|(not outer)
        if L in RL:
            hwm=np.where(esc,min(hw,NECK/2),hw)   # neck-down to NECK within the escape zone
            base=(dl>=hwm+cllv+M)&(dv>=hwm+clhv+M)&(dh>=hwm+0.25+M)&(de>=hwm+0.4)&(hvp_ok|(dvp>=hwm+HVCL+M))&es_ok
            ok[L]=(base&(px>=hwm+0.2+PM))|(base&(STUB[L][r0:r1,c0:c1]==v))|((onp==v)&(px>=hwm+0.2+PM))|((pd==v)&(px>=hwm+0.2+PM))|((pd==v)&padctr[L][r0:r1,c0:c1]); ESCM[L]=esc
            if L==1: ok[L]&=IN1OK[r0:r1,c0:c1] if NETS[v-1]!='GND' else False
        # vias always keep full clearance and never land on a pad (no via-in-pad: hand assembly)
        dp=ndimage.distance_transform_edt(pd==0)*RES if (pd!=0).any() else np.full(o.shape,99.0)
        via_ok&=(dl>=0.3+cllv+M)&(dv>=0.3+clhv+M)&(dp>=0.3+M)&(px>=0.3+0.2+PM)&(hvp_ok|(dvp>=0.3+HVCL+M))&es_vok
    return ok,via_ok,ESCM
LASTCONG=[]   # congested cells (layer, row, col) on the last path astar returned (negotiated mode)
def astar(v,src,dst,r0,r1,c0,c1,base_only=False):
    global LASTCONG
    ok,via_ok,escm=fields(v,r0,r1,c0,c1,base_only); h_,w_=r1-r0,c1-c0
    PEN=None
    if NEG and base_only is False:
        # negotiated congestion: other nets' routed copper is passable at a price that grows each pass (PRES) and with
        # the cell's conflict history; only fixed copper and the rules against it stay hard
        okb,vob,_=fields(v,r0,r1,c0,c1,np.array([0]))
        for L in RL: okb[L]&=(dyn[L][r0:r1,c0:c1]==0)|(dyn[L][r0:r1,c0:c1]==v)   # never through another net's copper
        for L in range(4): vob&=(dyn[L][r0:r1,c0:c1]==0)|(dyn[L][r0:r1,c0:c1]==v)
        PEN=np.concatenate([np.where(okb[L]&~ok[L],PRES*(1.0+HIST[L][r0:r1,c0:c1]),0.0).ravel() for L in RL]).tolist()
        VPEN=np.where(vob&~via_ok,PRES*3.0,0.0).ravel().tolist()
        ok,via_ok=okb,vob
    tgt=np.zeros((h_,w_),bool); goal={}
    for (L,ys,xs) in dst:
        sel=(ys>=r0)&(ys<r1)&(xs>=c0)&(xs<c1)
        if L in RL and sel.any():
            k=goal.setdefault(L,np.zeros((h_,w_),bool)); k[ys[sel]-r0,xs[sel]-c0]=True; tgt|=k
    if not tgt.any(): return None
    hd=ndimage.distance_transform_edt(~tgt)   # heuristic in cells
    # flat state k = layer slot * N + cell; bytes/lists index far faster than dicts and numpy scalars in this loop.
    # The heuristic is weighted (HW): paths may run a little long, the search expands far fewer cells
    N=h_*w_; nl=len(RL)
    G=b''.join((goal[L].ravel() if L in goal else np.zeros(N,bool)).tobytes() for L in RL)
    OK=b''.join(ok[L].ravel().tobytes() for L in RL); VO=via_ok.ravel().tobytes(); HD=(hd.ravel()*HW).tolist()
    LC=[{0:1.0,1:2.0,2:1.15,3:1.0}[L] for L in RL]
    # Kelvin pairs (/SH*_P with /SH*_N): steps within 0.6 mm of the partner leg's routed copper cost half, so the second
    # leg runs alongside the first
    pv=PARTNER.get(v); NEAR=None
    if pv:
        NEAR=b''.join(((ndimage.distance_transform_edt(dyn[L][r0:r1,c0:c1]!=pv)*RES<=0.6).ravel().tobytes()) for L in RL)
    INF=1e18; best=[INF]*(nl*N); prev=[-1]*(nl*N); pq=[]
    for (L,ys,xs) in src:
        if L not in RL: continue
        li=RL.index(L); sel=(ys>=r0)&(ys<r1)&(xs>=c0)&(xs<c1)
        for y,x in zip(ys[sel]-r0,xs[sel]-c0):
            k=li*N+int(y)*w_+int(x)
            if best[k]==INF: best[k]=0.0; heapq.heappush(pq,(HD[k-li*N],0.0,k))
    steps=[(-w_,1.0),(w_,1.0),(-1,1.0),(1,1.0),(-w_-1,1.414),(-w_+1,1.414),(w_-1,1.414),(w_+1,1.414)]
    n=0
    while pq:
        f,gc,k=heapq.heappop(pq)
        if gc>best[k]: continue
        if G[k]:
            path=[]; LASTCONG=[]
            while k>=0:
                path.append((RL[k//N],k%N))
                if PEN is not None and PEN[k]>0: LASTCONG.append((RL[k//N],k%N//w_+r0,k%N%w_+c0))
                k=prev[k]
            return path[::-1],w_,r0,c0,{L:escm[L].ravel() for L in RL}
        n+=1
        if n>1_500_000: return None
        li,i=divmod(k,N); base=li*N; x=i%w_; lc=LC[li]
        for d,c in steps:
            j=i+d
            if j<0 or j>=N: continue
            dx=(j%w_)-x
            if dx>1 or dx<-1: continue
            t=base+j
            if OK[t]:
                ng=gc+c*lc*(0.5 if (NEAR and NEAR[t]) else 1.0)+(PEN[t] if PEN is not None else 0.0)
                if ng<best[t]: best[t]=ng; prev[t]=k; heapq.heappush(pq,(ng+HD[j],ng,t))
        if VO[i]:
            for l2 in range(nl):
                if l2!=li:
                    t=l2*N+i
                    if OK[t]:
                        ng=gc+30+(VPEN[i] if PEN is not None else 0.0)
                        if ng<best[t]: best[t]=ng; prev[t]=k; heapq.heappush(pq,(ng+HD[i],ng,t))
    return None
def stamp(v,path,w_,r0,c0,esc):
    """draw the route into the owner grids and return polylines and vias in mm"""
    hw=width(NETS[v-1])/2; rad=int(math.ceil(hw/RES)); vr=int(math.ceil(0.3/RES))
    yy,xx=np.ogrid[-max(rad,vr):max(rad,vr)+1,-max(rad,vr):max(rad,vr)+1]
    disk=lambda r: (yy*yy+xx*xx)<=r*r
    polys=[]; vias=[]; cur=None; viacells=[]
    for k,(L,i) in enumerate(path):
        y,x=divmod(i,w_); Y,Xc=y+r0,x+c0
        if k and path[k-1][0]!=L:
            vias.append([X0+Xc*RES,Y0+Y*RES])
            d=disk(vr); R=d.shape[0]//2
            for LL in range(4):
                sub=dyn[LL][Y-R:Y+R+1,Xc-R:Xc+R+1]; bs=own[LL][Y-R:Y+R+1,Xc-R:Xc+R+1]
                if sub.shape==d.shape: sub[d&(sub==0)&(bs==0)]=v
            dynhole[Y,Xc]=v; viacells.append((Y,Xc)); cur=None
        wk=min(hw,NECK/2) if esc[L][i] else hw
        if cur is None or cur[0]!=L or cur[2]!=wk:
            last=cur[1][-1] if (cur is not None and cur[0]==L) else None
            cur=[L,[last] if last else [],wk]; polys.append(cur)
        cur[1].append((Xc,Y))
        d=disk(int(math.ceil(wk/RES))); R=d.shape[0]//2; sub=dyn[L][Y-R:Y+R+1,Xc-R:Xc+R+1]; bs=own[L][Y-R:Y+R+1,Xc-R:Xc+R+1]
        if sub.shape==d.shape: sub[d&(sub==0)&(bs==0)]=v
    out=[]
    for L,pts,wk in polys:
        if len(pts)<2: continue
        simp=[pts[0]]
        for a,b_,c in zip(pts,pts[1:],pts[2:]):
            if (b_[0]-a[0],b_[1]-a[1])!=(c[0]-b_[0],c[1]-b_[1]): simp.append(b_)
        simp.append(pts[-1])
        def mm(x,y):
            if STUB[L][y,x]==v:
                sx,sy=SNAPX[L][y,x],SNAPY[L][y,x]
                return [sx if sx==sx else X0+x*RES, sy if sy==sy else Y0+y*RES]
            return [X0+x*RES,Y0+y*RES]
        out.append(dict(layer=L,width=2*wk,pts=[mm(x,y) for x,y in simp]))
    return out,vias,viacells
def rip(u):
    """remove every committed connection of net u"""
    for L in range(4): dyn[L][dyn[L]==u]=0
    dynhole[dynhole==u]=0; ROUTES.pop(u,None)
# ---- main loop
def bbox(cells):
    ys=np.concatenate([c[1] for c in cells]); xs=np.concatenate([c[2] for c in cells]); return ys.min(),ys.max(),xs.min(),xs.max()
todo=[]
for n in NETS:
    if n.startswith('unconnected-'): continue
    v=NID[n]; comps=islands(v)
    if len(comps)>1:
        b_=[bbox(c) for c in comps]; todo.append(((max(b[1] for b in b_)-min(b[0] for b in b_))+(max(b[3] for b in b_)-min(b[2] for b in b_)),n))
KELV=re.compile(r'/SH\w*_[PN]$')
PARTNER={}
for n in NETS:
    if KELV.match(n):
        o=n[:-1]+('N' if n.endswith('P') else 'P')
        if o in NID: PARTNER[NID[n]]=NID[o]
# Kelvin legs first (P before N), so they take the short direct paths before the board fills; then shortest nets first
todo.sort(key=lambda t:(0 if KELV.match(t[1]) else 1, (0 if t[1].endswith('_P') else 1) if KELV.match(t[1]) else 0, t[0]))
print('nets to route',len(todo),flush=True)
fails=[]; t0=time.time(); nconn=0; tries={}; queue=[n for _,n in todo]; nrip=0; ripped={}
TMAX=float(os.environ.get('TMAX','3000'))   # wall-clock budget in s; whatever is still queued then is reported open
MAXRIP=3                                     # a net is ripped at most this often, so rip-up cannot cycle
def norip(u): return NETS[u-1]=='GND' or len(ROUTES.get(u,[]))>6 or ripped.get(u,0)>=MAXRIP
def dump():
    open_=sorted({n for n in set(fails)|set(queue) if len(islands(NID[n]))>1})
    routes=[dict(net=NETS[v-1],tracks=[s for rec in recs for s in rec['segs']],vias=[q for rec in recs for q in rec['vias_mm']]) for v,recs in ROUTES.items()]
    json.dump(dict(routes=routes,failed=open_),open(sys.argv[2],'w')); return open_
def connect(v,base_only=False):
    comps=islands(v)
    if len(comps)<2: return 'done'
    comps.sort(key=lambda c:-sum(len(x[1]) for x in c))
    A=comps[0]; rest=comps[1:]
    ba=bbox(A); cy,cx=(ba[0]+ba[1])/2,(ba[2]+ba[3])/2
    rest.sort(key=lambda c: (lambda b:((b[0]+b[1])/2-cy)**2+((b[2]+b[3])/2-cx)**2)(bbox(c)))
    bb_=bbox(rest[0])
    for m in ((60,) if v in PARTNER else (60,300)):   # a Kelvin leg never detours far: it fails and rips up instead
        r0=max(0,min(ba[0],bb_[0])-m); r1=min(H,max(ba[1],bb_[1])+m+1); c0=max(0,min(ba[2],bb_[2])-m); c1=min(W,max(ba[3],bb_[3])+m+1)
        res=astar(v,A,[x for c in rest for x in c],r0,r1,c0,c1,base_only)
        if res: return res
    return None
def blockers(v,res):
    """nets whose routed copper sits within clearance reach of this path (base-only route); a via counts on all layers"""
    path,w_,r0,c0,esc=res; reach=int(math.ceil((width(NETS[v-1])/2+0.6+0.4)/RES)); hit=set()
    prevL=None
    for L,i in path:
        y,x=divmod(i,w_); Y,X=y+r0,x+c0
        lays=range(4) if (prevL is not None and prevL!=L) else (L,)
        for LL in lays:
            win=dyn[LL][max(0,Y-reach):Y+reach+1,max(0,X-reach):X+reach+1]
            hit.update(int(a) for a in np.unique(win) if a and a!=v)
        prevL=L
    return hit
# ---- negotiated congestion (PathFinder): every net routes each pass, crossing other nets' clearance at a price that
# rises each pass and with each cell's conflict history; the next pass reroutes only nets still in conflict. What is
# left after NITER passes goes to the strict sequential loop below (rip-up, then reported open)
HIST=[np.zeros((H,W),np.float32) for _ in range(4)]
if NEG:
    order={n:k for k,(_,n) in enumerate(todo)}; it=0
    while queue and time.time()-t0<TMAX and it<NITER:
        conf=set(); hard=[]
        for n in queue:
            v=NID[n]; rip(v)
            while True:
                res=connect(v)
                if res=='done': break
                if res is None: hard.append(n); break
                cong=list(LASTCONG); path,w_,r0,c0,esc=res; segs,vias,vc=stamp(v,path,w_,r0,c0,esc)
                ROUTES.setdefault(v,[]).append(dict(segs=segs,vias_mm=vias,vias=vc)); nconn+=1
                if cong:
                    conf.add(v); conf|=blockers(v,res)
                    for L,y,x in cong: HIST[L][y,x]+=1.0
        it+=1; dump()
        print(f'pass {it}: {len(queue)} nets routed, {len(conf)} in conflict, {len(hard)} blocked by fixed copper, pres {PRES:.1f}, {time.time()-t0:.0f} s',flush=True)
        queue=sorted({NETS[u-1] for u in conf}|set(hard),key=lambda n:order.get(n,1e9)); PRES*=1.6
    # strict finish: rip what is still in conflict and route it with hard clearances
    for n in queue: rip(NID[n])
    NEG=False
while queue and time.time()-t0<TMAX:
    n=queue.pop(0); v=NID[n]
    while True:
        res=connect(v)
        if res=='done': break
        if res is None:
            tries[n]=tries.get(n,0)+1
            alt=connect(v,base_only=np.array([u for u in ROUTES if norip(u)]+[0])) if tries[n]<=3 else None
            if alt is None or alt=='done':
                fails.append(n); print("FAIL",n,flush=True); break
            hit={u for u in blockers(v,alt) if not norip(u)}
            for u in hit:
                ripped[u]=ripped.get(u,0)+1
                rip(u); nrip+=1
                if NETS[u-1] not in queue: queue.append(NETS[u-1])
            res=alt
        path,w_,r0,c0,esc=res; segs,vias,vc=stamp(v,path,w_,r0,c0,esc)
        ROUTES.setdefault(v,[]).append(dict(segs=segs,vias_mm=vias,vias=vc)); nconn+=1
        if nconn%100==0: dump()
        if nconn%25==0: print(f'{nconn} connections, {len(fails)} failed, {nrip} rip-ups, queue {len(queue)}, {time.time()-t0:.0f} s',flush=True)
fails=dump()
print(f'done: {nconn} connections committed, {nrip} rip-ups, nets still open {len(fails)}: {fails}, {time.time()-t0:.0f} s')
