#!/usr/bin/env python3
"""Stage 3: bring Freerouting's session back into the board.
KiCad's SES import replaces every track and via with the session's wiring, which also carries the fixed power-stage copper
and the hatch traces tools/dsn_prep.py used to stand in for the pours. This deletes exactly those hatch traces (from
<dsn>.hatch.json: layer, net, y and x-span of each 0.5 mm scan line), keeps the rest, unlocks the signal tracks, refills
the zones (the GND fills on L1, In2 and L4 come back around the new signals) and saves.
Run with KiCad's Python: <kicad python> tools/ses_import.py <board.kicad_pcb> <session.ses> <dsn.hatch.json> <out.kicad_pcb>"""
import collections,json,sys
import pcbnew
b=pcbnew.LoadBoard(sys.argv[1])
W=pcbnew.FromMM(0.5); um=lambda v: round(pcbnew.ToMM(v)*1000)
n0=sum(1 for _ in b.GetTracks())
PRE0={(t.GetNetname(),t.GetLayer(),um(t.GetStart().x),um(t.GetStart().y),um(t.GetEnd().x),um(t.GetEnd().y)) for t in b.GetTracks()}
assert pcbnew.ImportSpecctraSES(b,sys.argv[2]),'SES import failed'
H=collections.defaultdict(list)
for L,n,x0,y,x1 in json.load(open(sys.argv[3])): H[(L,n,y)].append((x0,x1))
gone=0; kept=collections.Counter()
TR0=[t for t in b.GetTracks()]
for t in TR0:
    if t.GetClass()!='PCB_TRACK' or t.GetWidth()!=W: continue
    s,e=t.GetStart(),t.GetEnd()
    if abs(um(s.y)-um(e.y))>2: continue
    k=(b.GetLayerName(t.GetLayer()),t.GetNetname(),um(s.y))
    spans=H.get(k) or H.get((k[0],k[1],k[2]-1)) or H.get((k[0],k[1],k[2]+1))
    if spans and any(min(um(s.x),um(e.x))>=a-5 and max(um(s.x),um(e.x))<=c+5 for a,c in spans):
        b.Remove(t); gone+=1; t.SetWidth(0)   # marks it; KiCad 10's Tracks() proxy is stale after a Remove()
ALL=[t for t in TR0 if t.GetClass()=='PCB_VIA' or t.GetWidth()!=0]
key=lambda t:(t.GetNetname(),t.GetLayer(),um(t.GetStart().x),um(t.GetStart().y),um(t.GetEnd().x),um(t.GetEnd().y))
# LOCK=<regex>: the scripted critical pre-routes (Kelvin pairs, gate drives, UVLO) stay locked for hand finishing
import os,re
LOCK=re.compile(os.environ.get('LOCK','^$'))
for t in ALL: t.SetLocked(bool(LOCK.match(t.GetNetname())) and key(t) in PRE0)
on_in1=[t for t in ALL if t.GetClass()=='PCB_TRACK' and t.GetLayer()==pcbnew.In1_Cu]
print('tracks on In1 (should be none; they cut the GND plane):',len(on_in1),sorted({t.GetNetname() for t in on_in1})[:20])
pcbnew.ZONE_FILLER(b).Fill(b.Zones())
b.Save(sys.argv[4])
n1=len(ALL)
print(f'tracks+vias before {n0}, after import and hatch removal {n1}; hatch removed {gone}; saved {sys.argv[4]}')
