#!/usr/bin/env python3
"""Rev B5 placement: puts every part on the front of the 100 x 150 mm board, in the blocks below (pdb/design/LAYOUT_RULES_REVB.md).
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
# ---- blocks: (X0,X1,Y0,Y1) in floorplan coordinates. Rev B5 (docs: pdb/design/LAYOUT_RULES_REVB.md):
# single-sided; pack pair on the top edge; flow RS1 -> bank A -> bank B (upper fan) -> bus caps -> port breakers -> pigtails
BLK=dict(
 soft=(-44.0,-11.0,311.0,328.4), bankactl=(17.8,25.0,268.0,302.4), clamp=(29.0,42.5,254.0,272.0),
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
put('clamp','R53 R46 D4 Q6 C49 R23 R24 TP8')
put('out','U3 U4 R18 R19 R20 R21 R22 R64 C8 C9 R65 R66 C48')
put('soft','RS9 Q7 U7 R25 R26 C19 C20 TP5 C14 C15 F2 R51 D15 Q12 R60')
put('out','U17 C30 R47 D18 RS8 U6 C16 C17 C18 TP4 C41 F3 F4 F5 D20 U9 RS10 U10 R38 R39 R40 R41 R42 R43 C31 C32 C33 C34 C35 C36 D5 D19 U18 TP9 R44 R45 C37 D6 D13 R67 U25 R128 R129 R130 R131 Q21 C80 C81 D41')
PORT={1:('RS2','U11','C21','U19','Q15',68,50,23),2:('RS3','U12','C22','U20','Q16',78,55,26),3:('RS4','U13','C23','U21','Q17',88,60,29),
      4:('RS5','U14','C24','U22','Q18',98,65,32),5:('RS6','U15','C25','U23','Q19',108,70,35),6:('RS7','U16','C26','U24','Q20',118,75,38)}
for k,(rs,ina,cd,u,q,r0,c0,d0) in PORT.items():
    put(f'L{k}',' '.join([rs,ina,cd,u,q]+[f'R{r0+i}' for i in range(10)]+[f'C{c0+i}' for i in range(5)]+[f'D{d0+i}' for i in range(3)]))
FIX={}   # ref: (X,Y,rot,back); back is never used: the board is single-sided (Joey, 2026-11-07)
FIX['J1']=(8.0,335.0,0,False); FIX['J3']=(29.0,333.9,0,False); FIX['J25']=(37.0,333.9,0,False); FIX['J24']=(-30.0,334.0,0,False)
FIX['RS1']=(0.0,308.0,0,False); FIX['D7']=(9.2,307.5,90,False); FIX['D21']=(16.0,307.5,90,False)
# absorber: six 10 x 10 cans in a 2 x 3 block right of the pack keep-out (X -5..25, Y 315-340), D7 pair beside RS1
for k,r in enumerate(('C38','C44','C45','C82','C83','C84','C85','C86')): FIX[r]=(30.75+11.5*(k%2),320.7-13.8*(k//2),90,False); FIX['D1']=(-14.5,334.5,90,False)
FIX['Q8']=(-9.0,296.0,0,False); FIX['Q9']=(9.0,296.0,0,False); FIX['Q1']=(-9.0,276.0,0,False); FIX['Q2']=(9.0,276.0,0,False)
for k,r in enumerate(('C10','C11','C12','C13','C47')): FIX[r]=(-23.2+k*11.6,260.5,90,False)
for Y,r in ((244.7,'J8'),(227.2,'J10'),(209.7,'J12')): FIX[r]=(45.0,Y,90,False)
for Y,r in ((244.7,'J9'),(227.2,'J11'),(209.7,'J13')): FIX[r]=(-45.0,Y,90,False)
for X,r in ((-31.0,'J21'),(-19.0,'J5'),(-10.5,'J6'),(-2.0,'J7'),(6.5,'J23'),(17.5,'J22')): FIX[r]=(X,195.0,0,False)
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
GAP=0.25  # courtyards already carry their own margin
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
TIE=[(-7.0,336.0),(23.0,336.0)]+[(s*45.0,Y-8.75) for s in (-1,1) for Y in (244.7,227.2,209.7)]  # one hole per pair: the tie loops through it and round the board edge
for X,Y in TIE:
    for d in (0,):
        h=pcbnew.FootprintLoad(FPDIR+'MountingHole.pretty','MountingHole_2.2mm_M2'); h.SetReference(f'TH{len([f for f in b.GetFootprints() if f.GetReference().startswith("TH")])+1}')
        b.Add(h); h.SetPosition(P(X,Y))
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
