#!/usr/bin/env python3
"""Stage 3 (own router), step 3: add the tracks and vias from tools/grid_route.py to the board, refill the zones (the GND
fills on L1, In2 and L4 close round the new signals) and save.
Layers in routes.json: 0 F.Cu, 2 In2, 3 B.Cu. Vias are 0.6 mm / 0.3 mm.
The router joins islands on a 0.1 mm grid, so a route can stop on the raster edge of its target: at the edge of a
same-net track (0.1 mm off its centreline) or just outside a pad. Such ends are extended here with one short segment
to the track's centreline or the pad's centre (inside copper the router already cleared), so KiCad sees them joined.
Run with KiCad's Python: <kicad python> tools/route_import.py <board.kicad_pcb> <routes.json> <out.kicad_pcb>"""
import json,math,sys
import pcbnew
b=pcbnew.LoadBoard(sys.argv[1]); R=json.load(open(sys.argv[2]))
LAY={0:pcbnew.F_Cu,1:pcbnew.In1_Cu,2:pcbnew.In2_Cu,3:pcbnew.B_Cu}
P=lambda x,y: pcbnew.VECTOR2I(pcbnew.FromMM(x),pcbnew.FromMM(y))
nt=nv=0; new=[]
for r in R['routes']:
    net=b.FindNet(r['net'])
    for t in r['tracks']:
        for a,c in zip(t['pts'],t['pts'][1:]):
            s=pcbnew.PCB_TRACK(b); s.SetStart(P(*a)); s.SetEnd(P(*c)); s.SetWidth(pcbnew.FromMM(t['width'])); s.SetLayer(LAY[t['layer']]); s.SetNet(net); b.Add(s); nt+=1; new.append(s)
    for x,y in r['vias']:
        v=pcbnew.PCB_VIA(b); v.SetPosition(P(x,y)); v.SetDrill(pcbnew.FromMM(0.3)); v.SetWidth(pcbnew.FromMM(0.6)); v.SetNet(net); b.Add(v); nv+=1
# ---- join raster-edge ends
def segd(p,a,c):
    ex,ey=c.x-a.x,c.y-a.y; L=ex*ex+ey*ey or 1
    t=max(0.0,min(1.0,((p.x-a.x)*ex+(p.y-a.y)*ey)/L)); q=pcbnew.VECTOR2I(int(a.x+t*ex),int(a.y+t*ey))
    return math.hypot(p.x-q.x,p.y-q.y),q
tracks=[t for t in b.Tracks() if t.GetClass()=='PCB_TRACK']; vias=[t for t in b.Tracks() if t.GetClass()=='PCB_VIA']
bynet={}
for t in tracks: bynet.setdefault(t.GetNetCode(),[]).append(t)
vianet={}
for v in vias: vianet.setdefault(v.GetNetCode(),[]).append(v.GetPosition())
pads={}
for p in b.GetPads(): pads.setdefault(p.GetNetCode(),[]).append(p)
zones=[z for z in b.Zones() if not z.GetIsRuleArea()]
tol=pcbnew.FromMM(0.01); added=0; left=[]
ID=lambda t: t.m_Uuid.AsString()   # SWIG proxies of one track are different Python objects
for s in new:
    nc=s.GetNetCode(); L=s.GetLayer(); sid=ID(s)
    for e in (s.GetStart(),s.GetEnd()):
        if any(ID(u)!=sid and u.GetLayer()==L and (u.GetStart()==e or u.GetEnd()==e) for u in bynet[nc]): continue
        if e in vianet.get(nc,[]): continue
        if any(p.IsOnLayer(L) and p.HitTest(e) for p in pads.get(nc,[])): continue
        if any(ID(u)!=sid and u.GetLayer()==L and segd(e,u.GetStart(),u.GetEnd())[0]<u.GetWidth()/2-tol for u in bynet[nc]): continue
        if any(z.GetNetCode()==nc and z.IsOnLayer(L) and z.HitTestFilledArea(L,e) for z in zones): continue
        tgt=None
        for p in pads.get(nc,[]):
            if p.IsOnLayer(L) and p.HitTest(e,pcbnew.FromMM(0.25)): tgt=p.GetPosition(); break
        if tgt is None:
            best=None
            for u in bynet[nc]:
                if ID(u)==sid or u.GetLayer()!=L: continue
                d,q=segd(e,u.GetStart(),u.GetEnd())
                if d<=u.GetWidth()/2+pcbnew.FromMM(0.06) and (best is None or d<best[0]): best=(d,q)
            if best: tgt=best[1]
        if tgt is None: left.append((s.GetNetname(),pcbnew.ToMM(e.x),pcbnew.ToMM(e.y))); continue
        j=pcbnew.PCB_TRACK(b); j.SetStart(e); j.SetEnd(tgt); j.SetWidth(s.GetWidth()); j.SetLayer(L); j.SetNet(s.GetNet()); b.Add(j)
        bynet[nc].append(j); added+=1
pcbnew.ZONE_FILLER(b).Fill(b.Zones())
b.Save(sys.argv[3])
print(f'added {nt} track segments, {nv} vias, {added} end joins; open ends left {len(left)}: {left[:20]}')
print(f'failed nets from the router: {R["failed"]}')
