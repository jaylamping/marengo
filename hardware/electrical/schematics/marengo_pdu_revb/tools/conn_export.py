#!/usr/bin/env python3
"""Connector data for tools/package_step.py: courtyard box, rotation and numbered pads (position, net) of every J*, in
floorplan coordinates (X -50..50, Y 190..340, as in tools/place.py).
Run with KiCad's Python: <kicad python> tools/conn_export.py <board.kicad_pcb> <conn.json>"""
import json,sys
import pcbnew
OX,OY=100,100
def F(v): return round(pcbnew.ToMM(v.x)-OX-50,3),round(OY+340-pcbnew.ToMM(v.y),3)
b=pcbnew.LoadBoard(sys.argv[1]); out={}
for f in b.GetFootprints():
    r=f.GetReference()
    if not r.startswith('J'): continue
    cy=f.GetCourtyard(pcbnew.F_CrtYd); bb=cy.BBox() if cy.OutlineCount() else f.GetBoundingBox(False,False)
    x0,y1=F(bb.GetOrigin()); x1,y0=F(bb.GetEnd())
    out[r]=dict(box=[x0,x1,y0,y1],rot=round(f.GetOrientationDegrees()),fp=f.GetFPIDAsString(),value=f.GetValue(),
                pads=[[p.GetNumber(),*F(p.GetPosition()),p.GetNetname()] for p in f.Pads() if p.GetNumber() not in ('','MP')])
json.dump(out,open(sys.argv[2],'w'),indent=1)
print(len(out),'connectors')
