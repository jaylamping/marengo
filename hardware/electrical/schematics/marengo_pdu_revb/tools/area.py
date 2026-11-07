#!/usr/bin/env python3
"""Courtyard area against the board. Run with KiCad's Python: <kicad python> tools/area.py <net.net>
Same courtyard-bbox method as rev A's fit test. Fill limit 60 %."""
import os,sys
sys.path.insert(0,os.path.dirname(__file__)); from sx import *
import pcbnew
MM=pcbnew.pcbIUScale.IU_PER_MM if hasattr(pcbnew,'pcbIUScale') else 1e6
FPDIR=os.environ.get('KICAD_FP',next(p for p in ('/Applications/KiCad/KiCad.app/Contents/SharedSupport/footprints/','C:/Program Files/KiCad/10.0/share/kicad/footprints/') if os.path.isdir(p)))
N=parse(open(sys.argv[1]).read())
rows=[]
for c in getall(get(N,'components'),'comp'):
    ref=str(get(c,'ref')[1]); f=get(c,'footprint'); f=str(f[1]) if f else ''
    if not f: continue
    d=any(str(get(p,'name')[1])=='dnp' for p in getall(c,'property'))
    lib,name=f.split(':')
    fp=pcbnew.FootprintLoad('marengo_revb.pretty' if lib=='marengo_revb' else FPDIR+lib+'.pretty',name)
    cy=fp.GetCourtyard(pcbnew.F_CrtYd); bb=cy.BBox() if cy.OutlineCount() else fp.GetBoundingBox(False)
    a=bb.GetWidth()/MM*bb.GetHeight()/MM
    rows.append((a,ref,str(get(c,'value')[1])[:30],d,any(p.HasHole() for p in fp.Pads())))
rows.sort(reverse=True)
import json
if len(sys.argv)>2: json.dump([dict(a=r[0],ref=r[1],val=r[2],dnp=r[3],tht=r[4]) for r in rows],open(sys.argv[2],'w'))
BOARD=100*150
tot=sum(r[0] for r in rows); fit=sum(r[0] for r in rows if not r[3]); tht=sum(r[0] for r in rows if r[4]); 
print('parts',len(rows),'fitted',sum(1 for r in rows if not r[3]))
print(f'courtyard all {tot:.0f} mm2, without legs {fit:.0f} mm2')
print(f'blocks both faces (drilled) {tht:.0f} mm2')
print(f'one face {BOARD} mm2, two faces {2*BOARD} mm2, 60 % of two faces {0.6*2*BOARD:.0f} mm2')
print(f'needed board at 60 %: {tot/0.6:.0f} mm2 of face area, so {tot/(0.6*2*BOARD)*100:.0f} % of the 60 % budget two-sided, {tot/(0.6*BOARD)*100:.0f} % one-sided')
for r in rows[:16]: print(f'{r[0]:8.0f} {r[1]:6} {r[2]} {"DNP" if r[3] else ""}')
