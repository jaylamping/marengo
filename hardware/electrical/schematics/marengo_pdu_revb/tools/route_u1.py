#!/usr/bin/env python3
"""U1 (LM5069, bank A) escapes, drawn by hand before any routing. The package puts UVLO (pin 3, E-stop) between SH_P
(pin 2) and the GND pins, and both sense pins face RS1, so the router's escape would run 54 V copper alongside the UVLO
pad just outside the courtyard. Here every escape is one straight segment out of the courtyard (the fine-pitch rule in
the .kicad_dru covers copper that enters it), clear of the 1.5 mm E-stop rule beyond:
  - SH_P: pin 2 straight into the inner edge of RS1's SH_P pad (2.8 mm);
  - SH_N: pin 1 out and down to the inner edge of RS1's SH_N pad (6.0 mm), away from pin 3;
  - UVLO: pin 3 inward under the body to a via; it leaves on B.Cu (the router takes it from there).
Positions are taken from the pads, so the script follows U1 if tools/place.py moves it.
Run with KiCad's Python: <kicad python> tools/route_u1.py <board.kicad_pcb>   (in place)"""
import sys
import pcbnew
b=pcbnew.LoadBoard(sys.argv[1])
u=b.FindFootprintByReference('U1'); rs=b.FindFootprintByReference('RS1')
pad=lambda f,n: next(p for p in f.Pads() if p.GetNumber()==n)
mm=lambda v: (pcbnew.ToMM(v.x),pcbnew.ToMM(v.y))
V=lambda x,y: pcbnew.VECTOR2I(pcbnew.FromMM(x),pcbnew.FromMM(y))
def track(net,pts,w=0.25,layer=pcbnew.F_Cu):
    for a,c in zip(pts,pts[1:]):
        t=pcbnew.PCB_TRACK(b); t.SetStart(V(*a)); t.SetEnd(V(*c)); t.SetWidth(pcbnew.FromMM(w)); t.SetLayer(layer); t.SetNet(b.FindNet(net)); t.SetLocked(True); b.Add(t)
p1,p2,p3=(mm(pad(u,n).GetPosition()) for n in '123'); s2,s3=mm(pad(rs,'2').GetPosition()),mm(pad(rs,'3').GetPosition())
yb=lambda p:(pcbnew.ToMM(p.GetBoundingBox().GetY()),pcbnew.ToMM(p.GetBoundingBox().GetBottom()))   # pad's Y extent on the board
(t2,b2),(t3,b3)=yb(pad(rs,'2')),yb(pad(rs,'3'))
dx=1 if s2[0]>p2[0] else -1   # RS1 to the right of U1's sense column (tools/place.py)
ex=s2[0]-dx*0.2               # inner edge of RS1's sense pads, 0.2 mm in
track('/SH_P',[p2,(ex,min(max(p2[1],t2+0.3),b2-0.04))])   # level with pin 2 (0.2 mm from pin 3), into the pad's sense end
track('/SH_N',[p1,(p1[0]+dx*1.45,p1[1]+1.6),(ex,t3+0.3)])
vx=p3[0]-dx*2.1
track('/UVLO',[p3,(vx,p3[1])])
v=pcbnew.PCB_VIA(b); v.SetPosition(V(vx,p3[1])); v.SetDrill(pcbnew.FromMM(0.3)); v.SetWidth(pcbnew.FromMM(0.6)); v.SetNet(b.FindNet('/UVLO')); v.SetLocked(True); b.Add(v)
pcbnew.ZONE_FILLER(b).Fill(b.Zones())
b.Save(sys.argv[1]); print('U1 escapes: SH_P, SH_N, UVLO via at',round(vx,2),round(p3[1],2))
