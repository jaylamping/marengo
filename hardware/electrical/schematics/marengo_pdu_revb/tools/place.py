#!/usr/bin/env python3
"""Rev B4 placement: puts every part on the 100 x 150 mm board in the floorplan blocks (tools/floorplan.py, 8e7d2df8).
Run with KiCad's Python: <kicad python> tools/place.py <net.net> <out.kicad_pcb> <placement.json>
Board coordinates follow the floorplan: X -50..50 (robot's left = +X), Y 190..340 (up). No routing."""
import os,sys,json
sys.path.insert(0,os.path.dirname(__file__)); from sx import *
import pcbnew
FPDIR=os.environ.get('KICAD_FP',next(p for p in ('/Applications/KiCad/KiCad.app/Contents/SharedSupport/footprints/','C:/Program Files/KiCad/10.0/share/kicad/footprints/') if os.path.isdir(p)))
HERE=os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
N=parse(open(sys.argv[1]).read())
OX,OY=100,100                       # page offset, mm
def P(X,Y): return pcbnew.VECTOR2I(pcbnew.FromMM(OX+X+50),pcbnew.FromMM(OY+340-Y))
b=pcbnew.BOARD(); b.SetCopperLayerCount(4)
# ---- blocks: (X0,X1,Y0,Y1) in floorplan coordinates
T=13.16
BLK=dict(
 soft=(-36.5,-12.0,305,339), latch=(-12.0,10.4,305,339), latch2=(18.6,22.4,305,325.8), clamp=(22.6,36.5,305,325.8),
 bankB=(-36.5,9.6,268,302), bankA=(9.6,36.5,268,302),
 midm=(-24.1,-10.0,191.5,213), spare=(-24.1,24.1,250.8,263.5), out=(-24.1,24.1,214,236.8),
 limbR=(-36.5,-24.6,203,264), limbL=(24.6,36.5,203,264))
G={}
def put(blk,refs):
    for r in refs.split(): G[r]=blk
put('bankB','Q1 Q2 RS1 U1 R1 R2 R3 R4 R5 R6 C3 C4 D2 Q3 D1 C1 C2 D7 C38 R46 R53 R54 R55 C43 R56 Q13')
put('bankA','Q8 Q9 U8 R27 R28 R29 R30 R31 R32 C27 C28 R33 R34 TP6 R35 R36 R37 C29 D12 D17 R52 C40 R61 R62 C46')
put('midm','U17 C30 R47 D18')
put('latch','U2 C5 R7 R8 TP1 R9 R10 R11 R12 R13 R14 R15 R16 R17 C6 C7 Q4 Q5 TP2 J20 D3 F1 C39 D8 D9 D10 D11 R59 C42 D14 R48 R49 R50 Q10 Q11 D16 R57 R58 Q14 R44 R45 C37 D6 D13')
put('clamp','U3 U4 R18 R19 R20 R21 R22 R23 R24 R64 C8 C9 D4 Q6')
put('soft','RS9 Q7 U7 R25 R26 C19 C20 TP5 C14 C15 F2 R51 D15 Q12 R60')
put('limbR','RS3 U12 C22 RS5 U14 C24 RS7 U16 C26')
put('limbL','RS2 U11 C21 RS4 U13 C23 RS6 U15 C25')
put('spare','U9 RS10 U10 R38 R39 R40 R41 R42 R43 C31 C32 C33 C34 C35 C36 D5 D19 J22 J23 U18 R63 TP3')
put('out','RS8 U6 C16 C17 C18 TP4 J5 J6 J7 J21 C41 F3 F4 F5 D20')
FIX={}   # ref: (X,Y,rot,back)
for Y,r in ((291.2,'J1'),(277.8,'J9'),(252.6,'J11'),(239.2,'J13'),(225.8,'J15')): FIX[r]=(-43.4,Y,0,False)
for Y,r in ((318.0,'J3'),(304.6,'J4'),(291.2,'J2'),(277.8,'J8'),(252.6,'J10'),(239.2,'J12'),(225.8,'J14')): FIX[r]=(43.4,Y,0,False)
FIX['J25']=(30.2,332.4,0,False); FIX['J26']=(17.0,332.4,0,False)
FIX['J24']=(-43.1,314.2,90,False)
for k,r in enumerate(('C10','C11','C12','C13')): FIX[r]=(-18.3+k*12.2,244.1,90,False)
FIX['C45']=(-30.8,275.0,0,True); FIX['C44']=(16.6,316.0,90,False)
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
for r,(X,Y,rot,back) in FIX.items(): place_at(fps[r],X,Y,rot,back)
rep={}
GAP=0.5
def maxrects(W,H,items):
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
for blk,(X0,X1,Y0,Y1) in BLK.items():
    refs=[r for r,g in G.items() if g==blk]
    items=[]
    for r in refs:
        fp=fps[r]; bb=bbox(fp); items.append((pcbnew.ToMM(bb.GetWidth()),pcbnew.ToMM(bb.GetHeight()),r))
    items.sort(key=lambda i:-i[0]*i[1])
    res=maxrects(X1-X0,Y1-Y0,items); over=[]
    for w,h,r in items:
        q=res[r]
        if q is None: over.append(r); continue
        x,y,ww,hh,rot=q
        if rot: fps[r].SetOrientationDegrees(90)
        place_at(fps[r],X0+x+ww/2,Y1-y-hh/2)
    used=sum(w*h for w,h,r in items); area=(X1-X0)*(Y1-Y0)
    rep[blk]=dict(parts=len(items),used=round(used),fill=round(used/area*100),overflow=over)
# anything that didn't fit goes to a parking strip below the board edge, so it's visible
px=-50
for blk in rep:
    for r in rep[blk]['overflow']:
        bb=bbox(fps[r]); w=pcbnew.ToMM(bb.GetWidth()); place_at(fps[r],px+w/2,184); px+=w+1
missing=[r for r in fps if r not in G and r not in FIX]
# ---- outline, holes, keep-outs, notes
def seg(layer,a,c,w=0.15):
    s=pcbnew.PCB_SHAPE(b); s.SetShape(pcbnew.SHAPE_T_RECT); s.SetLayer(layer); s.SetWidth(pcbnew.FromMM(w))
    s.SetStart(P(*a)); s.SetEnd(P(*c)); b.Add(s); return s
def txt(layer,X,Y,t,sz=1.2):
    x=pcbnew.PCB_TEXT(b); x.SetText(t); x.SetLayer(layer); x.SetPosition(P(X,Y)); x.SetTextSize(pcbnew.VECTOR2I(pcbnew.FromMM(sz),pcbnew.FromMM(sz))); b.Add(x)
seg(pcbnew.Edge_Cuts,(-50,340),(50,190),0.1)
for sx_ in (-1,1):
    for Y in (194,265,336):
        h=pcbnew.FootprintLoad(FPDIR+'MountingHole.pretty','MountingHole_3.2mm_M3_Pad'); h.SetReference(f'H{len([f for f in b.GetFootprints() if f.GetReference().startswith("H")])+1}')
        gnd=nets.get(('J2','1'));
        b.Add(h); h.SetPosition(P(sx_*46,Y))
for a,c,t in (((-40,304),(40,264),'Fan zone (upper)'),((-40,233),(40,193),'Fan zone (lower)')):
    seg(pcbnew.Cmts_User,a,c); txt(pcbnew.Cmts_User,a[0]+12,a[1]-1.5,t)
for (a,c,t) in (((-36.5,302),(9.6,268),'Thermal: bank B drain pour MID >= 20 cm2, all 4 layers, 30+ vias/tab'),
                ((9.6,302),(36.5,268),'Thermal: bank A drain pour SW_D >= 20 cm2, all 4 layers, 30+ vias/tab')):
    seg(pcbnew.User_1,a,c,0.3); txt(pcbnew.User_1,a[0]+1,c[1]+1.5,t,1.0)
for blk,(X0,X1,Y0,Y1) in BLK.items():
    seg(pcbnew.User_2,(X0,Y1),(X1,Y0),0.1); txt(pcbnew.User_2,X0+0.8,Y1-0.8,blk,0.9)
ds=b.GetDesignSettings()
b.Save(sys.argv[2])
json.dump(dict(blocks=rep,missing=missing),open(sys.argv[3],'w'),indent=1)
print(json.dumps(rep,indent=0)); print('missing',missing)
