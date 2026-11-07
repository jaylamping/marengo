#!/usr/bin/env python3
"""Stage 2c, before signal routing: E-stop guard spacing (LAYOUT_RULES_REVB.md "E-stop safety nets").
Every ESTOP_SAFE pad must keep >= 1.5 mm to 54 V copper on its layer. Placement packs parts by courtyard, so a few
E-stop parts landed next to 54 V parts. For each footprint with such a pad this pass tries the smallest move (0.25 mm grid,
up to 4 mm) of the E-stop part, then of the 54 V part if that part has no pad in a pour, that clears every E-stop/54 V
pair it touches, keeps courtyards from overlapping (0.1 mm), and stays inside the board.
Pairs it can't clear are listed for a hand move or a guard. Reuses tools/route_checks.py's net lists.
Run with KiCad's Python, in place: <kicad python> tools/hv_spacing.py <board.kicad_pcb>"""
import itertools,re,sys
import pcbnew
b=pcbnew.LoadBoard(sys.argv[1]); mm=pcbnew.FromMM
pcbnew.ZONE_FILLER(b).Fill(b.Zones())
src=open(__file__.replace('hv_spacing.py','route_checks.py'),encoding='utf-8').read()
ns={'re':re,'b':b}; exec(src[src.index('ESTOP=set'):src.index('LAYERS=')],ns)
ESTOP,HV=ns['ESTOP'],ns['HV']
L=pcbnew.F_Cu
def hvcopper(exclude=None):
    out=[]
    for p in b.GetPads():
        if HV(p.GetNetname()) and p.IsOnLayer(L) and p.GetParentFootprint()!=exclude: out.append((p.GetEffectiveShape(L),p.GetParentFootprint()))
    for t in b.GetTracks():
        if HV(t.GetNetname()) and t.IsOnLayer(L) and (t.GetClass()!='PCB_VIA' or t.FlashLayer(L)): out.append((t.GetEffectiveShape(L),None))
    for z in b.Zones():
        if HV(z.GetNetname()) and z.IsOnLayer(L) and not z.GetIsRuleArea():
            fp=z.GetFilledPolysList(L)
            if fp.OutlineCount(): out.append((fp,None))
    return out
def espads(fp): return [p for p in fp.Pads() if p.GetNetname() in ESTOP and p.IsOnLayer(L)]
def hvpads(fp): return [p for p in fp.Pads() if HV(p.GetNetname()) and p.IsOnLayer(L)]
def viol(fp,hv):
    return [(p,o) for p in espads(fp) for s,o in hv if o!=fp and s.Collide(p.GetEffectiveShape(L),mm(1.5)-1)]
def crt(fp):
    c=fp.GetCourtyard(pcbnew.F_CrtYd); return c if c.OutlineCount() else None
EDGE=b.GetBoardEdgesBoundingBox()
def ok_place(fp):
    c=crt(fp)
    if c is None: return True
    bb=c.BBox()
    if not EDGE.Contains(bb.GetOrigin()) or not EDGE.Contains(bb.GetEnd()): return False
    for g in b.GetFootprints():
        if g==fp: continue
        d=crt(g)
        if d is not None and c.Collide(d.Outline(0),mm(0.1)): return False
    return True
def wired(fp):
    """parts with power-stage copper on their pads (ties, in-pad vias, the brake pair) stay where they are"""
    for t in b.GetTracks():
        for p in fp.Pads():
            if any(p.HitTest(q) for q in ((t.GetPosition(),) if t.GetClass()=='PCB_VIA' else (t.GetStart(),t.GetEnd()))): return True
    return False
RMAX=int(4/0.25)
STEPS=sorted({(dx,dy) for dx in range(-RMAX,RMAX+1) for dy in range(-RMAX,RMAX+1)},key=lambda d:d[0]**2+d[1]**2)[1:]
NOMOVE={'U1'}   # bank B's hot-swap controller keeps its sense and gate loops; its neighbours move instead
def try_move(fp,check):
    p0=fp.GetPosition()
    for dx,dy in STEPS:
        fp.SetPosition(pcbnew.VECTOR2I(p0.x+mm(0.25*dx),p0.y+mm(0.25*dy)))
        if ok_place(fp) and check(): return (0.25*dx,-0.25*dy)
    fp.SetPosition(p0); return None
moved=[]
for rnd in range(4):
  if rnd==3: RMAX=int(6/0.25); STEPS[:]=sorted({(dx,dy) for dx in range(-RMAX,RMAX+1) for dy in range(-RMAX,RMAX+1)},key=lambda d:d[0]**2+d[1]**2)[1:]
  left=[]; hv=hvcopper()
  for fp in list(b.GetFootprints()):
      if not viol(fp,hv): continue
      v=viol(fp,hv)
      d=None if wired(fp) or fp.GetReference() in NOMOVE else try_move(fp,lambda: not viol(fp,hv))
      if d: moved.append((fp.GetReference(),d)); continue
      # move the 54 V neighbour(s) instead, if they are free parts (no pad in a pour, not fixed by the power stage)
      others={o.GetReference():o for _,o in v if o is not None}.values()
      done=True
      for o in others:
          if o.IsLocked() or wired(o) or o.GetReference() in NOMOVE or any(z.HitTestFilledArea(L,p.GetPosition()) for p in hvpads(o) for z in b.Zones() if z.GetNetCode()==p.GetNetCode()):
              done=False; continue
          def chk(o=o):
              h2=hvcopper(); return not [1 for p in espads(fp) for s,q in h2 if q==o and s.Collide(p.GetEffectiveShape(L),mm(1.5)-1)]
          d=try_move(o,chk)
          if d: moved.append((o.GetReference(),d)); hv=hvcopper()
          else: done=False
      if not done or viol(fp,hv): left.append((fp.GetReference(),sorted({p.GetNumber()+':'+p.GetNetname() for p,_ in viol(fp,hv)}),sorted({o.GetReference() if o else 'pour/track/via' for _,o in viol(fp,hv)})))
# pours and vias the moves can't clear: keep pours 1.5 mm off each remaining E-stop pad (rule area, no copper pour, F.Cu)
# and drop 54 V vias within 1.5 mm of it (they sit in arrays; the rest of the array carries the current)
pcbnew.ZONE_FILLER(b).Fill(b.Zones())
nko=nvia=0
for fp in b.GetFootprints():
    for p in espads(fp):
        s=p.GetEffectiveShape(L)
        for t in list(b.GetTracks()):
            if t.GetClass()=='PCB_VIA' and HV(t.GetNetname()) and t.GetEffectiveShape(L).Collide(s,mm(1.5)-1): b.Remove(t); nvia+=1
        if any(z.IsOnLayer(L) and HV(z.GetNetname()) and not z.GetIsRuleArea() and z.GetFilledPolysList(L).Collide(s,mm(1.5)-1) for z in b.Zones()):
            bb=p.GetBoundingBox(); g=mm(1.6)
            k=pcbnew.ZONE(b); k.SetIsRuleArea(True); k.SetDoNotAllowZoneFills(True); k.SetDoNotAllowTracks(False); k.SetDoNotAllowVias(False)
            k.SetDoNotAllowPads(False); k.SetDoNotAllowFootprints(False); k.SetLayer(L); k.SetZoneName(f'ESTOP_GUARD_{fp.GetReference()}_{p.GetNumber()}')
            o=k.Outline(); o.NewOutline()
            for x,y in ((bb.GetLeft()-g,bb.GetTop()-g),(bb.GetRight()+g,bb.GetTop()-g),(bb.GetRight()+g,bb.GetBottom()+g),(bb.GetLeft()-g,bb.GetBottom()+g)): o.Append(pcbnew.VECTOR2I(x,y))
            b.Add(k); nko+=1
print('guard keep-outs',nko,'54 V vias removed',nvia)
# 54 V vias in the port VBUS strips that crowd low-voltage pads on L1 (IPC-2221B 0.6 mm): drop them, the strips keep the rest
def lvdom(n): return bool(re.match(r'/(G\d|GA\d|GATE|GATE_A|G_L\d|GPD_L\d|GPU_L\d|BST_L\d|GC_L\d)$',n))
nlv=0
for t in list(b.GetTracks()):
    if t.GetClass()!='PCB_VIA' or not HV(t.GetNetname()) or not t.FlashLayer(L): continue
    s=t.GetEffectiveShape(L)
    if any(p.IsOnLayer(L) and not HV(p.GetNetname()) and not lvdom(p.GetNetname()) and p.GetNetname() not in ESTOP and s.Collide(p.GetEffectiveShape(L),mm(0.6)-1) for p in b.GetPads()):
        b.Remove(t); nlv+=1
print('54 V vias within 0.6 mm of low-voltage pads removed',nlv)
pcbnew.ZONE_FILLER(b).Fill(b.Zones())
left=[(fp.GetReference(),sorted({p.GetNumber()+':'+p.GetNetname() for p,_ in viol(fp,hvcopper())})) for fp in b.GetFootprints() if viol(fp,hvcopper())]
pcbnew.ZONE_FILLER(b).Fill(b.Zones())
b.Save(sys.argv[1])
print('moved',len(moved),moved); print('left',len(left))
for r in left: print('  ',r)
