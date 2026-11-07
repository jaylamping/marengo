#!/usr/bin/env python3
"""Rev B5 stage 2b: the +12V_RAW feed from J24 (buck board return, top-left) down to RS8 at the 12 V block, about 105 mm at
9.1 A expected, 11.6 A ceiling (REV_B_PARTS.md 12 V budget). In2 carries the whole run (6-7 mm, 1 oz); L4 doubles the
left leg and the top of the cross band (2 oz), stitched to In2. The path stays clear of the via fields: down the left side
outside the latch parts' via-free column, across Y 259-266 between the left port fields and the VBUS row, then down
X -7..-1 to RS8. Freerouting drops the vias from RS8/C17/C16 to the strip.
Run with KiCad's Python, in place: <kicad python> tools/route_12v.py <board.kicad_pcb>   (idempotent)"""
import sys
import pcbnew
OX,OY=100,100
def P(X,Y): return pcbnew.VECTOR2I(pcbnew.FromMM(OX+X+50),pcbnew.FromMM(OY+340-Y))
b=pcbnew.LoadBoard(sys.argv[1]); N=b.FindNet('/+12V_RAW')
for z in list(b.Zones()):
    if z.GetNetname()=='/+12V_RAW': b.Remove(z)
for t in list(b.GetTracks()):
    if t.GetNetname()=='/+12V_RAW': b.Remove(t)
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
zone(pcbnew.In2_Cu,TOP+[(-40.5,-33.5,258.8,331.4),(-40.5,-1.2,258.8,265.8),(-7.2,-1.2,228.0,265.8)],'+12V_RAW In2')
zone(pcbnew.B_Cu,TOP+[(-40.5,-33.5,255.5,331.4),(-40.5,-14.0,255.5,261.0)],'+12V_RAW L4')
n=0
def via(X,Y):
    global n
    v=pcbnew.PCB_VIA(b); v.SetPosition(P(X,Y)); v.SetDrill(pcbnew.FromMM(0.3)); v.SetWidth(pcbnew.FromMM(0.6)); v.SetNet(N); b.Add(v); n+=1
for Y in range(262,329,3):
    for X in (-38.5,-35.5): via(X,Y)
for X in range(-32,-14,2): via(X,259.9)
pcbnew.ZONE_FILLER(b).Fill(b.Zones())
b.Save(sys.argv[1]); print('+12V_RAW zones on In2 and L4,',n,'stitch vias')
