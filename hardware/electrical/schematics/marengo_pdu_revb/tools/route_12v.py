#!/usr/bin/env python3
"""Rev B5 stage 2b: the +12V_RAW feed from J24 (buck board return, top-left) down to RS8 at the 12 V block, about 105 mm at
9.1 A expected, 11.6 A ceiling (REV_B_PARTS.md 12 V budget). In2 carries the whole run (6-7 mm, 1 oz); L4 doubles the
left leg and the top of the cross band (2 oz), stitched to In2. The path stays clear of the via fields: down the left side
outside the latch parts' via-free column, across Y 259-266 between the left port fields and the VBUS row, then down
X -7..-1 to RS8. Freerouting drops the vias from RS8/C17/C16 to the strip.
Run with KiCad's Python, in place: <kicad python> tools/route_12v.py <board.kicad_pcb>   (idempotent)"""
import os,sys
import pcbnew
YB=float(os.environ.get('PDU_YBOT','170'))   # board bottom edge (tools/place.py); option A puts RS8 in the bottom band
OX,OY=100,100
def P(X,Y): return pcbnew.VECTOR2I(pcbnew.FromMM(OX+X+50),pcbnew.FromMM(OY+340-Y))
b=pcbnew.LoadBoard(sys.argv[1]); N=b.FindNet('/+12V_RAW')
# KiCad 10's SWIG proxies go stale after any Remove() in a session, so an old strip is removed, saved, and the script
# re-runs itself on a fresh load
old=[t for t in b.GetTracks() if t.GetNetname()=='/+12V_RAW']+[z for z in b.Zones() if z.GetNetname()=='/+12V_RAW']
if old:
    for o in old: b.Remove(o)
    b.Save(sys.argv[1]); import subprocess; sys.exit(subprocess.call([sys.executable]+sys.argv))
TRK=[t for t in b.GetTracks()]
def zone(layer,rects,name):
    z=pcbnew.ZONE(b); z.SetLayer(layer); z.SetNet(N); z.SetAssignedPriority(6); z.SetZoneName(name)
    z.SetLocalClearance(pcbnew.FromMM(0.5)); z.SetMinThickness(pcbnew.FromMM(0.25))
    z.SetPadConnection(pcbnew.ZONE_CONNECTION_FULL); z.SetIslandRemovalMode(pcbnew.ISLAND_REMOVAL_MODE_ALWAYS)
    o=z.Outline()
    for (x0,x1,y0,y1) in rects:
        o.NewOutline()
        for X,Y in ((x0,y0),(x1,y0),(x1,y1),(x0,y1)): o.Append(P(X,Y))
    o.Simplify(); b.Add(z)
TOP=[(-40.5,-21.0,328.6,331.4),(-30.0,-21.0,331.4,334.4)]           # band under J24's GND row, fingers to pins 10-12
if YB>=190:   # 100 x 150 board
    zone(pcbnew.In2_Cu,TOP+[(-40.5,-33.5,258.8,331.4),(-40.5,-1.2,258.8,265.8),(-7.2,-1.2,228.0,265.8)],'+12V_RAW In2')
    zone(pcbnew.B_Cu,TOP+[(-40.5,-33.5,255.5,331.4),(-40.5,-14.0,255.5,261.0)],'+12V_RAW L4')
    LEG=[(-38.0,Y) for Y in range(262,329,3)]+[(-35.0,Y) for Y in range(262,329,3)]
else:
    # 100 x 170 board: an In2 + L4 leg under the latch and soft start left no room for their vias (the router could not
    # escape U7's pins). The left leg is now L4 only (2 oz, 5.4 mm: about 3 mOhm, 0.3 W at 9.1 A) along the left edge,
    # clear of the M3 hole at (-46, 265); a short L4 stub joins it to the cross band, which stays In2 + L4 as before.
    # The stub overlaps the band (Y 260-261) so L4 is one polygon (two separate outlines kept only one after Simplify,
    # which left the leg unfilled), and the leg vias sit where the stub and the In2 band overlap
    TOPA=[(-49.4,-21.0,328.6,331.4),(-30.0,-21.0,331.4,334.4)]
    zone(pcbnew.In2_Cu,[(-40.5,-1.2,258.8,265.8),(-7.2,-1.2,YB+12.0,265.8)],'+12V_RAW In2')
    zone(pcbnew.B_Cu,TOPA+[(-49.4,-44.0,269.6,331.4),(-44.6,-37.0,260.0,271.0),(-40.5,-14.0,255.5,261.0)],'+12V_RAW L4')
    LEG=[(-39.8,Y) for Y in (262.8,264.6)]+[(-38.0,Y) for Y in (262.8,264.6)]
n=0
PADS=[p for p in b.GetPads()]
def free(v):
    """the stitch via must clear every pad, track and via of another net by 0.3 mm on F.Cu and B.Cu (the latch block sits
    on top of the left leg), and stay off holes"""
    for L in (pcbnew.F_Cu,pcbnew.B_Cu):
        s=v.GetEffectiveShape(L)
        for p in PADS:
            if p.GetNetCode()!=N.GetNetCode() and p.IsOnLayer(L) and s.Collide(p.GetEffectiveShape(L),pcbnew.FromMM(0.3)): return False
            if p.HasHole() and s.Collide(p.GetEffectiveHoleShape(),pcbnew.FromMM(0.3)): return False
        for t in TRK:
            if t!=v and t.GetNetCode()!=N.GetNetCode() and t.IsOnLayer(L) and s.Collide(t.GetEffectiveShape(L),pcbnew.FromMM(0.3)): return False
    return True
def inside(v):
    """a stitch via must land inside the In2 and the L4 outline (0.6 mm in), or it joins only one layer"""
    zs=[z for z in b.Zones() if z.GetNetname()=='/+12V_RAW']; c=v.GetPosition(); r=pcbnew.FromMM(0.6)
    pts=[c]+[pcbnew.VECTOR2I(c.x+dx,c.y+dy) for dx,dy in ((r,0),(-r,0),(0,r),(0,-r))]
    return all(any(z.IsOnLayer(L) and all(z.Outline().Contains(q) for q in pts) for z in zs) for L in (pcbnew.In2_Cu,pcbnew.B_Cu))
def via(X,Y):
    """first free spot within 1.5 mm of (X, Y) on a 0.25 mm grid"""
    global n
    v=pcbnew.PCB_VIA(b); v.SetDrill(pcbnew.FromMM(0.3)); v.SetWidth(pcbnew.FromMM(0.6)); v.SetNet(N)
    for dx,dy in sorted({(i*0.25,j*0.25) for i in range(-6,7) for j in range(-6,7)},key=lambda d:d[0]**2+d[1]**2):
        v.SetPosition(P(X+dx,Y+dy))
        if inside(v) and free(v) and not any(t.GetClass()=='PCB_VIA' and t.GetNetCode()==N.GetNetCode() and (t.GetPosition()-v.GetPosition()).EuclideanNorm()<pcbnew.FromMM(1.2) for t in TRK):
            b.Add(v); TRK.append(v); n+=1; return
for X,Y in LEG: via(X,Y)
for X in range(-32,-14,2): via(X,259.9)
pcbnew.ZONE_FILLER(b).Fill(b.Zones())
b.Save(sys.argv[1]); print('+12V_RAW zones on In2 and L4,',n,'stitch vias')
