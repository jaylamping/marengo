#!/usr/bin/env python3
"""Rev B5 placement: puts every part on the front of the 100 x 170 mm board (option A, Joey 2026-11-07), in the blocks below (pdb/design/LAYOUT_RULES_REVB.md).
Run with KiCad's Python: <kicad python> tools/place.py <net.net> <out.kicad_pcb> <placement.json>
Board coordinates follow the floorplan: X -50..50 (robot's left = +X), Y 170..340 (up). No routing."""
import os,sys,json
sys.path.insert(0,os.path.dirname(__file__)); from sx import *
import pcbnew
FPDIR=os.environ.get('KICAD_FP',next(p for p in ('/Applications/KiCad/KiCad.app/Contents/SharedSupport/footprints/','C:/Program Files/KiCad/10.0/share/kicad/footprints/') if os.path.isdir(p)))
HERE=os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
N=parse(open(sys.argv[1]).read())
OX,OY=100,100                       # page offset, mm
def P(X,Y): return pcbnew.VECTOR2I(pcbnew.FromMM(OX+X+50),pcbnew.FromMM(OY+340-Y))
b=pcbnew.BOARD(); b.SetCopperLayerCount(4)
# board bottom edge: 170 is the 100 x 170 board (option A, Joey 2026-11-07: routing room); PDU_YBOT=190 rebuilds the old
# 100 x 150 placement, whose control blocks were too dense to route (74-84 % fill). The extra 20 mm go to
# the bottom, under the ports, so the pack entry, absorber, banks, heatsink and ports keep their coordinates
YB=float(os.environ.get('PDU_YBOT','170')); WIDE=YB<190
# ---- blocks: (X0,X1,Y0,Y1) in floorplan coordinates. Rev B5 (docs: pdb/design/LAYOUT_RULES_REVB.md):
# single-sided; pack pair on the top edge; flow RS1 -> bank A -> bank B (upper fan) -> bus caps -> port breakers -> pigtails
BLK=dict(
 soft=(-44.0,-11.0,311.0,328.4), bankactl=(17.8,25.0,268.0,301.2), clamp=(29.0,42.5,254.0,272.0),
 bankctl=(-28.5,-17.8,268.0,310.5), latch=(-42.0,-29.0,254.0,310.5),
 L1=(9.5,42.5,236.0,253.5), L3=(9.5,42.5,218.5,236.0), L5=(9.5,42.5,201.0,218.5),
 L2=(-42.5,-9.5,236.0,253.5), L4=(-42.5,-9.5,218.5,236.0), L6=(-42.5,-9.5,201.0,218.5),
 out=(-9.0,9.0,201.0,253.5))
G={}
def put(blk,refs):
    for r in refs.split(): G[r]=blk
put('bankctl','R63 TP3 C1 R54 R55 R56 C43 Q13 R33 R34 D12 D17 R52 TP6 C2 U1 R1 R2 R3 R4 R5 R6 C3 C4 D2 Q3 TP10 TP12')
put('latch','U2 C5 R7 R8 TP1 J20 R9 D3 R10 R11 C6 R12 Q4 Q5 R13 R14 R15 R16 R17 C7 TP2 F1 C39 D8 D9 D10 D11 R59 C42 D14 R48 R49 R50 Q10 Q11 D16 R57 R58 Q14 D22 TP7')
put('bankactl','U8 C27 C28 C29 R27 R28 R29 R30 R31 R32 R35 R36 R37 C40 C46 R61 R62 TP11 TP13')
put('clamp','R46 D4 Q6 C49 R23 R24 TP8')
put('bankctl','TP14'); put('bankactl','TP15'); put('latch','TP16')   # hand-assembly probe pads (SW_D, ABS, WDO_N)
put('out','U3 U4 R18 R19 R20 R21 R22 R64 C8 C9 R65 R66 C48')
put('soft','Q7 U7 R25 R26 C19 C20 TP5 C14 C15 F2 R51 D15 Q12 R60')
put('out','U17 C30 R47 D18 RS8 U6 C16 C17 C18 TP4 C41 F3 F4 F5 D20 U9 RS10 U10 R38 R39 R40 R41 R42 R43 C31 C32 C33 C34 C35 C36 D5 D19 U18 TP9 R44 R45 C37 D6 D13')
put('latch','U25 R128 R129 R130 R131 Q21 C80 C81 D41 R67')   # watchdog beside the latch it drops (L_E, Q5)
PF='U26 U27 R132 R133 R134 R135 R136 R137 R138 C93 C94 C95 Q22 D42'   # power-fail warning (block 5b): beside the 12 V block and J21
put('out',PF)
if WIDE:
    # option A: the control blocks spread to about 50 % fill. The latch and soft start take the left edge (J20 fixed there,
    # the M3 hole reserved), bank B's controller takes the latch's old column, and a new low-voltage band along the new
    # bottom edge takes the 12 V distribution (next to its Micro-Fit outputs) and the watchdog
    BLK.update(soft=(-48.0,-11.0,311.0,328.4),latch=(-48.0,-31.0,254.0,310.5),bankctl=(-30.5,-17.8,268.0,310.5),lv=(-48.0,48.0,YB+11.0,200.5))
    put('lv','U17 C30 R47 D18 RS8 U6 C16 C17 C18 TP4 C41 F3 F4 F5 D20 U9 RS10 U10 R38 R39 R40 R41 R42 R43 C31 C32 C33 C34 C35 C36 D5 D19 U18 TP9 R44 R45 C37 D6 D13')
    put('lv','U25 R128 R129 R130 R131 Q21 C80 C81 D41 R67'); put('lv',PF)
RES={}; PFIX={}
PORT={1:('RS2','U11','C21','U19','Q15',68,50,23),2:('RS3','U12','C22','U20','Q16',78,55,26),3:('RS4','U13','C23','U21','Q17',88,60,29),
      4:('RS5','U14','C24','U22','Q18',98,65,32),5:('RS6','U15','C25','U23','Q19',108,70,35),6:('RS7','U16','C26','U24','Q20',118,75,38)}
# block -> reserved rects (X0,X1,Y0,Y1), kept free of parts (via fields, pour necks)
for k,(rs,ina,cd,u,q,r0,c0,d0) in PORT.items():
    put(f'L{k}',' '.join([cd,u]+[f'R{r0+i}' for i in range(10)]+[f'C{c0+i}' for i in range(5)]+[f'D{d0+i}' for i in range(2)]))
    s=1 if k%2 else -1; X0,X1,Y0,Y1=BLK[f'L{k}']
    # right ports as drawn; left ports are the same row turned 180 degrees about the block centre, so the FET's gate pin
    # stays on the inner side and the source pins face the pigtail (a mirror image isn't possible single-sided)
    if s>0:
        PFIX[rs]=(14.6,Y1-2.15,0); PFIX[q]=(25.9,Y0+8.75,90); PFIX[f'D{d0+2}']=(35.4,Y0+5.0,90)
        # port controller straight under the shunt, its power-side row (sense, gate, V_L) facing it: Kelvin legs under
        # 8 mm; the meter under the controller
        PFIX[u]=(15.0,Y1-7.8,90); PFIX[ina]=(14.45,Y1-13.4,0)
        RES.setdefault('out',[]).append((6.8,9.0,Y1-7.5,Y1))   # VBUS via field at the shunt's bus pad, on the out block's edge
        RES[f'L{k}']=[(38.8,42.5,Y0,Y0+5.2)]   # V_L neck; the D_L copper runs in the gap between the shunt and FET courtyards
    else:
        PFIX[rs]=(-14.6,Y0+2.15,180); PFIX[q]=(-25.9,Y1-8.75,270); PFIX[f'D{d0+2}']=(-35.4,Y1-5.0,270)
        PFIX[u]=(-15.0,Y0+7.8,270); PFIX[ina]=(-14.45,Y0+13.4,180)
        RES.setdefault('out',[]).append((-9.0,-6.8,Y0,Y0+7.5))
        RES[f'L{k}']=[(-42.5,-38.8,Y1-5.2,Y1)]     # V_L neck from the output clamp to the pigtail
if WIDE:
    # each port's RUN_P resistors (EN and INP feeds, and the EN pull-down) sit in the out block, away from the port's
    # 54 V shunt and sense parts (E-stop 1.5 mm rule); the out block's VBUS via fields keep 1.6 mm round them
    for k,(rs,ina,cd,u,q,r0,c0,d0) in PORT.items(): put('out',' '.join(f'R{r0+i}' for i in (7,8,9)))
    RES['out']=[(x0-1.6,x1+1.6,y0-1.6,y1+1.6) for x0,x1,y0,y1 in RES['out']]
    RES.setdefault('bankactl',[]).append((17.8,19.4,289.5,301.2))   # 1.6 mm from bank A's Q9 tab (SW_D)
FIX={}   # ref: (X,Y,rot,back); back is never used: the board is single-sided (Joey, 2026-11-07)
FIX['J1']=(8.0,335.0,0,False); FIX['J3']=(29.0,333.9,0,False); FIX['J25']=(37.0,333.9,0,False); FIX['J24']=(-30.0,334.0,0,False)
# power flow (routing stage 1): RS1 vertical, BAT pad up, SW_D pad down, Kelvin pads on the left; D7/D21 between the BAT
# pour and the cans, ABS pads down; RS9 BAT end toward the pack pour; R53 (ABS_N-GND link) under the can stack
FIX['RS1']=(0.0,309.3,270,False); FIX['D7']=(14.8,308.4,90,False); FIX['D21']=(21.65,308.4,90,False)
FIX['RS9']=(-20.4,326.4,180,False); FIX['R53']=(45.5,270.3,0,False)
# bank A and bank B controllers beside their shunts (Kelvin legs under 8 mm): U1 in the GND gap left of RS1, sense pins
# toward its Kelvin pads, GATE toward Q8 (UVLO leaves through a via under the body); U7 sense pins up toward RS9
FIX['U1']=(-8.25,312.6,180,False); FIX['U7']=(-13.8,320.15,270,False)
FIX['C87']=(-8.6,307.6,90,False); FIX['C88']=(-13.1,307.6,90,False)   # bank A drain ceramics, SW_D pad down onto the drain pour
FIX['C89']=(5.8,312.4,90,False); FIX['C90']=(9.4,312.4,90,False); FIX['C91']=(-8.6,327.6,0,False); FIX['C92']=(-8.6,331.0,0,False)   # pack-entry ceramics on the BAT pour
# absorber: six 10 x 10 cans in a 2 x 3 block right of the pack keep-out (X -5..25, Y 315-340), D7 pair beside RS1
for k,r in enumerate(('C38','C44','C45','C82','C83','C84','C85','C86')): FIX[r]=(30.75+11.5*(k%2),320.7-13.8*(k//2),90,False); FIX['D1']=(-14.5,334.5,90,False)
FIX['Q8']=(-9.0,296.0,0,False); FIX['Q9']=(9.0,296.0,0,False); FIX['Q1']=(-9.0,276.0,0,False); FIX['Q2']=(9.0,276.0,0,False)
for k,r in enumerate(('C10','C11','C12','C13','C47')): FIX[r]=(-23.2+k*11.6,260.5,270,False)   # + pad up, toward bank B's sources
# heatsink (THERMAL_OPTION.md option D): 4 x M3 through the board into the finned block under the FET zone (X -33..33, Y 200-305)
# 2-point mount on the centre line (two screws clamp it flat and fix its rotation): one in the 8.6 mm gap between
# bank A and bank B (a hole in the MID pour), one at the bottom of the out block
HSINK=[(0.0,286.0),(0.0,204.0)]
for X,Y in HSINK:
    blk=next((b_ for b_,(x0,x1,y0,y1) in BLK.items() if x0<=X<=x1 and y0<=Y<=y1),None)
    if blk: RES.setdefault(blk,[]).append((X-3.4,X+3.4,Y-3.4,Y+3.4))
RES.setdefault('soft',[]).append((-16.0,-11.0,323.6,328.4))   # BAT neck from RS9 to D1 and the pack pour
for Y,r in ((244.7,'J8'),(227.2,'J10'),(209.7,'J12')): FIX[r]=(45.0,Y,90,False)
for Y,r in ((244.7,'J9'),(227.2,'J11'),(209.7,'J13')): FIX[r]=(-45.0,Y,270,False)   # + at the top, toward the turned FET's pins
for X,r in ((-31.0,'J21'),(-19.0,'J5'),(-10.5,'J6'),(-2.0,'J7'),(6.5,'J23'),(17.5,'J22')): FIX[r]=(X,YB+5.0,0,False)
if WIDE:
    FIX['J20']=(-45.25,303.2,270,False); G.pop('J20',None)        # E-stop GH at the -X edge, opening outward
    RES.setdefault('latch',[]).append((-50.0,-41.8,260.8,269.2))   # M3 hole H at (-46, 265)
    RES.setdefault('soft',[]).append((-50.0,-41.8,328.4,340.0))
    for s in (-1,1): RES.setdefault('lv',[]).append(tuple(sorted((s*42.0,s*50.0)))+(197.0,200.5))   # L5/L6 pigtail tie holes at (+-45, 200.95)
    FIX['F2']=(-24.31,320.67,0,False); G.pop('F2',None)   # THT fuse holder: pins at Y 319 and above, clear of the heatsink (Y 196.5-316.5)
# ---- nets and footprints
nets={}
for net in getall(get(N,'nets'),'net'):
    nm=str(get(net,'name')[1]); ni=pcbnew.NETINFO_ITEM(b,nm); b.Add(ni)
    for x in getall(net,'node'): nets[(str(get(x,'ref')[1]),str(get(x,'pin')[1]))]=ni
fps={}
for c in getall(get(N,'components'),'comp'):
    ref=str(get(c,'ref')[1]); f=str(get(c,'footprint')[1]); lib,name=f.split(':')
    fp=pcbnew.FootprintLoad(os.path.join(HERE,'marengo_revb.pretty') if lib=='marengo_revb' else FPDIR+lib+'.pretty',name)
    fp.SetReference(ref); fp.SetValue(str(get(c,'value')[1])[:40]); fp.SetFPID(pcbnew.LIB_ID(lib,name))
    for pad in fp.Pads():
        ni=nets.get((ref,pad.GetNumber()))
        if ni: pad.SetNet(ni)
    if any(str(get(p,'name')[1])=='dnp' for p in getall(c,'property')): fp.SetDNP(True)
    b.Add(fp); fps[ref]=fp
def bbox(fp):
    cy=fp.GetCourtyard(pcbnew.F_CrtYd if not fp.IsFlipped() else pcbnew.B_CrtYd)
    bb=cy.BBox() if cy.OutlineCount() else fp.GetBoundingBox(False,False)
    return bb
def place_at(fp,X,Y,rot=0,back=False):
    fp.SetPosition(P(X,Y))
    if rot: fp.SetOrientationDegrees(rot)
    if back and not fp.IsFlipped(): fp.Flip(fp.GetPosition(),pcbnew.FLIP_DIRECTION_LEFT_RIGHT)
    # centre the courtyard on (X,Y)
    bb=bbox(fp); c=bb.GetCenter(); t=P(X,Y)
    fp.Move(pcbnew.VECTOR2I(t.x-c.x,t.y-c.y))
FIX.update({r:(X,Y,rot,False) for r,(X,Y,rot) in PFIX.items()})
for r in FIX: G.pop(r,None)
for r,(X,Y,rot,back) in FIX.items(): place_at(fps[r],X,Y,rot,back)
rep={}
GAP=0.25  # courtyards already carry their own margin
BGAP={**{f'L{k}':0.0 for k in range(1,7)},'out':0.0,'soft':0.0,'latch':0.0}   # port blocks: courtyards touching (0.5 mm between 0603 bodies), hand-solderable
def maxrects(W,H,items,occ=(),GAP=GAP):
    """MaxRects, best short side fit, 0/90 rotation. items: (w,h,ref). Returns ref->(x,y,rot) with x,y the lower-left from the block's top-left going down."""
    free=[(0.0,0.0,W,H)]; out={}
    def split(fr,u):
        x,y,w,h=fr; ux,uy,uw,uh=u
        if ux>=x+w or ux+uw<=x or uy>=y+h or uy+uh<=y: return [fr]
        r=[]
        if ux>x: r.append((x,y,ux-x,h))
        if ux+uw<x+w: r.append((ux+uw,y,x+w-ux-uw,h))
        if uy>y: r.append((x,y,w,uy-y))
        if uy+uh<y+h: r.append((x,uy+uh,w,y+h-uy-uh))
        return r
    for u in occ:
        nf=[]
        for fr in free: nf+=split(fr,u)
        free=[a for a in nf if a[2]>0.3 and a[3]>0.3]
        free=[a for a in free if not any(a is not c and a[0]>=c[0]-1e-9 and a[1]>=c[1]-1e-9 and a[0]+a[2]<=c[0]+c[2]+1e-9 and a[1]+a[3]<=c[1]+c[3]+1e-9 for c in free)]
    for w0,h0,ref in items:
        best=None
        for (fx,fy,fw,fh) in free:
            for rot,(w,h) in ((0,(w0+GAP,h0+GAP)),(90,(h0+GAP,w0+GAP))):
                if w<=fw+1e-6 and h<=fh+1e-6:
                    sc=(min(fw-w,fh-h),fy,fx)
                    if best is None or sc<best[0]: best=(sc,fx,fy,w,h,rot)
        if best is None: out[ref]=None; continue
        _,x,y,w,h,rot=best; out[ref]=(x,y,w,h,rot)
        nf=[]
        for fr in free: nf+=split(fr,(x,y,w,h))
        free=[a for a in nf if not any(a is not c and a[0]>=c[0]-1e-9 and a[1]>=c[1]-1e-9 and a[0]+a[2]<=c[0]+c[2]+1e-9 and a[1]+a[3]<=c[1]+c[3]+1e-9 for c in nf)]
        free=list(dict.fromkeys(free))
    return out
# spacing margins (tools/domains.py): courtyards touch (about 0.3-0.5 mm pad to pad between 0603s), which breaks the
# 0.6 mm rule between 54 V domains and the 1.5 mm E-stop rule. Parts with a pad riding on 54 V get 0.3 mm more on each
# side, E-stop parts 0.6 mm; a block that then overflows is packed without them (tools/hv_spacing.py moves what's left)
import domains
def margin(fp):
    ns=[p.GetNetname() for p in fp.Pads()]
    if any(domains.is_estop(n) for n in ns): return 1.2
    if any(domains.rides(n) for n in ns): return 0.6
    return 0.0
def pack(blk,X0,X1,Y0,Y1,refs):
    items=[]
    for r in refs:
        fp=fps[r]; bb=bbox(fp); m=margin(fp); items.append((pcbnew.ToMM(bb.GetWidth())+m,pcbnew.ToMM(bb.GetHeight())+m,r))
    items.sort(key=lambda i:-i[0]*i[1])
    occ=[]
    for f in fps.values():   # fixed parts inside the block
        if f.GetReference() in FIX:
            bb=bbox(f); fx0=pcbnew.ToMM(bb.GetX())-OX-50; fx1=fx0+pcbnew.ToMM(bb.GetWidth()); fy1=340-(pcbnew.ToMM(bb.GetY())-OY); fy0=fy1-pcbnew.ToMM(bb.GetHeight())
            if fx1>X0 and fx0<X1 and fy1>Y0 and fy0<Y1: occ.append((fx0-X0-GAP/2,Y1-fy1-GAP/2,fx1-fx0+GAP,fy1-fy0+GAP))
    for (rx0,rx1,ry0,ry1) in RES.get(blk,[]): occ.append((rx0-X0,Y1-ry1,rx1-rx0,ry1-ry0))
    res=maxrects(X1-X0,Y1-Y0,items,occ,BGAP.get(blk,GAP)); over=[]
    if any(q is None for q in res.values()):   # no room for the margins: a uniform 0.35 mm gap, else pack as before
        items=[(pcbnew.ToMM(bbox(fps[r]).GetWidth()),pcbnew.ToMM(bbox(fps[r]).GetHeight()),r) for _,_,r in items]
        res=maxrects(X1-X0,Y1-Y0,items,occ,0.35)
        if any(q is None for q in res.values()): res=maxrects(X1-X0,Y1-Y0,items,occ,BGAP.get(blk,GAP)); print('no room for spacing in',blk)
        else: print('uniform 0.35 mm gap in',blk)
    for w,h,r in items:
        q=res[r]
        if q is None: over.append(r); continue
        x,y,ww,hh,rot=q
        if rot: fps[r].SetOrientationDegrees(90)
        place_at(fps[r],X0+x+ww/2,Y1-y-hh/2)
    used=sum(w*h for w,h,r in items); area=(X1-X0)*(Y1-Y0)
    return dict(parts=len(items),used=round(used),fill=round(used/area*100),overflow=over)
# 54 V strips (option A): in the latch and bank B controller blocks the few 54 V parts (BAT fuse and cap, MID and VBUS
# zeners, MID bias, the absorber option) go in a strip along the block's bottom edge, toward the power stage, 1.6 mm from
# the E-stop parts that fill the rest (E-stop 1.5 mm rule)
HVSTRIP={'latch','bankctl'} if WIDE else set()
for blk,(X0,X1,Y0,Y1) in BLK.items():
    refs=[r for r,g in G.items() if g==blk]
    hv=[r for r in refs if blk in HVSTRIP and any(domains.is_hv(p.GetNetname()) for p in fps[r].Pads()) and not any(domains.is_estop(p.GetNetname()) for p in fps[r].Pads()) and r not in FIX]
    if hv:
        a_=sum(pcbnew.ToMM(bbox(fps[r]).GetWidth())*pcbnew.ToMM(bbox(fps[r]).GetHeight()) for r in hv)
        hs=min(0.5*(Y1-Y0),max(a_*2.0/(X1-X0),max(min(pcbnew.ToMM(bbox(fps[r]).GetWidth()),pcbnew.ToMM(bbox(fps[r]).GetHeight())) for r in hv)+0.6))
        print(blk,'54 V strip',round(hs,1),'mm:',' '.join(hv)); r1=pack(blk,X0,X1,Y0,Y0+hs,hv); r2=pack(blk,X0,X1,Y0+hs+1.6,Y1,[r for r in refs if r not in hv])
        rep[blk]=dict(parts=r1['parts']+r2['parts'],fill=r2['fill'],hv_strip=round(hs,1),overflow=r1['overflow']+r2['overflow'])
    else: rep[blk]=pack(blk,X0,X1,Y0,Y1,refs)
# J20 (JST GH horizontal, E-stop loop) must open toward the -X edge (KO_estop); maxrects only knows its box, so turn it 180
# about the courtyard centre, which keeps the box
if round(fps['J20'].GetOrientationDegrees())==90: fps['J20'].Rotate(bbox(fps['J20']).GetCenter(),pcbnew.EDA_ANGLE(180,pcbnew.DEGREES_T))
# anything that didn't fit goes to a parking strip below the board edge, so it's visible
px=-50
for blk in rep:
    for r in rep[blk]['overflow']:
        bb=bbox(fps[r]); w=pcbnew.ToMM(bb.GetWidth()); place_at(fps[r],px+w/2,YB-6); px+=w+1
missing=[r for r in fps if r not in G and r not in FIX]
# ---- outline, holes, keep-outs, notes
def seg(layer,a,c,w=0.15):
    s=pcbnew.PCB_SHAPE(b); s.SetShape(pcbnew.SHAPE_T_RECT); s.SetLayer(layer); s.SetWidth(pcbnew.FromMM(w))
    s.SetStart(P(*a)); s.SetEnd(P(*c)); b.Add(s); return s
def txt(layer,X,Y,t,sz=1.2):
    x=pcbnew.PCB_TEXT(b); x.SetText(t); x.SetLayer(layer); x.SetPosition(P(X,Y)); x.SetTextSize(pcbnew.VECTOR2I(pcbnew.FromMM(sz),pcbnew.FromMM(sz))); b.Add(x)
seg(pcbnew.Edge_Cuts,(-50,340),(50,YB),0.1)
for sx_ in (-1,1):
    for Y in (YB+4,265,336):
        h=pcbnew.FootprintLoad(FPDIR+'MountingHole.pretty','MountingHole_3.2mm_M3_Pad'); h.SetReference(f'H{len([f for f in b.GetFootprints() if f.GetReference().startswith("H")])+1}')
        gnd=nets.get(('J2','1'));
        b.Add(h); h.SetPosition(P(sx_*46,Y))
TIE=[(-7.0,336.0),(23.0,336.0)]+[(s*45.0,Y-8.75) for s in (-1,1) for Y in (244.7,227.2,209.7)]  # one hole per pair: the tie loops through it and round the board edge
for k,(X,Y) in enumerate(HSINK):
    h=pcbnew.FootprintLoad(FPDIR+'MountingHole.pretty','MountingHole_3.2mm_M3'); h.SetReference(f'HS{k+1}'); b.Add(h); h.SetPosition(P(X,Y))
for X,Y in TIE:
    for d in (0,):
        h=pcbnew.FootprintLoad(FPDIR+'MountingHole.pretty','MountingHole_2.2mm_M2'); h.SetReference(f'TH{len([f for f in b.GetFootprints() if f.GetReference().startswith("TH")])+1}')
        b.Add(h); h.SetPosition(P(X,Y))
# copper keep-outs round the holes (rule areas, all copper layers): heatsink screws HS1/HS2 no copper within 2 mm of the
# hole (LAYOUT_RULES_REVB.md, the screw and the heatsink are not insulated from it); tie holes and the M3 mounting pads
# 0.6 mm (54 V pours and tracks otherwise close to 0.25 mm, IPC-2221B 0.6 mm for 51-100 V)
import math
def keepout(X,Y,r,name):
    k=pcbnew.ZONE(b); k.SetIsRuleArea(True); k.SetDoNotAllowZoneFills(True); k.SetDoNotAllowTracks(True); k.SetDoNotAllowVias(True)
    k.SetDoNotAllowPads(False); k.SetDoNotAllowFootprints(False); k.SetLayerSet(pcbnew.LSET.AllCuMask()); k.SetZoneName(name)
    o=k.Outline(); o.NewOutline()
    r/=math.cos(math.pi/32)   # the 32-gon's flats sit at r
    for i in range(32): o.Append(P(X+r*math.cos(i*math.pi/16),Y+r*math.sin(i*math.pi/16)))
    b.Add(k)
for k,(X,Y) in enumerate(HSINK): keepout(X,Y,1.6+2.0,f'KO_HS{k+1}')
for k,(X,Y) in enumerate(TIE): keepout(X,Y,1.1+0.6,f'KO_TH{k+1}')
for sx_ in (-1,1):
    for Y in (YB+4,265,336): keepout(sx_*46,Y,3.2+0.6,f'KO_H_{sx_*46}_{Y:g}')
for a,c,t in (((-40,304),(40,264),'Fan zone (upper)'),((-40,233),(40,193),'Fan zone (lower)')):
    seg(pcbnew.Cmts_User,a,c); txt(pcbnew.Cmts_User,a[0]+12,a[1]-1.5,t)
for (a,c,t) in (((-15,304),(15,287),'Thermal: bank A drain SW_D, all 4 layers (2 oz), 30 vias/tab'),
                ((-15,286),(15,266),'Thermal: bank B drain MID, all 4 layers (2 oz), 30 vias/tab')):
    seg(pcbnew.User_1,a,c,0.3); txt(pcbnew.User_1,a[0]+1,c[1]+1.5,t,1.0)
for blk,(X0,X1,Y0,Y1) in BLK.items():
    seg(pcbnew.User_2,(X0,Y1),(X1,Y0),0.1); txt(pcbnew.User_2,X0+0.8,Y1-0.8,blk,0.9)
ds=b.GetDesignSettings()
# courtyard overlaps between placed footprints (DRC proper still runs in pcbnew)
bbs=[(f.GetReference(),bbox(f)) for f in b.GetFootprints() if not f.GetReference().startswith(('H','TH'))]
clash=[]
for i1 in range(len(bbs)):
    for i2 in range(i1+1,len(bbs)):
        r1,a1=bbs[i1]; r2,a2=bbs[i2]
        if a1.Intersects(a2):
            ov=a1.Intersect(a2)
            if pcbnew.ToMM(ov.GetWidth())>0.05 and pcbnew.ToMM(ov.GetHeight())>0.05: clash.append((r1,r2,round(pcbnew.ToMM(ov.GetWidth()),2),round(pcbnew.ToMM(ov.GetHeight()),2)))
back=[f.GetReference() for f in b.GetFootprints() if f.IsFlipped()]
b.Save(sys.argv[2])
json.dump(dict(blocks=rep,missing=missing,overlaps=clash,back=back),open(sys.argv[3],'w'),indent=1)
print(json.dumps(rep,indent=0)); print('missing',missing); print('overlaps',len(clash),clash[:40]); print('on the back:',back)
