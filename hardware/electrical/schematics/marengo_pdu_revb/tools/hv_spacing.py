#!/usr/bin/env python3
"""Stage 2c, before signal routing: 54 V and E-stop guard spacing between parts (LAYOUT_RULES_REVB.md).
Every E-stop pad keeps >= 1.5 mm to 54 V copper, and 54 V pads keep 0.6 mm to pads of other voltage domains, on F.Cu
(tools/domains.py). Placement packs parts by courtyard (0.5 mm between 0603 bodies), so some parts land too close. For each footprint with such a pad this pass tries the smallest move (0.25 mm grid,
up to 4 mm) of the E-stop part, then of the 54 V part if that part has no pad in a pour, that clears every E-stop/54 V
pair it touches, keeps courtyards from overlapping (0.1 mm), and stays inside the board.
Pairs it can't clear are listed for a hand move or a guard.
Run with KiCad's Python, in place: <kicad python> tools/hv_spacing.py <board.kicad_pcb>"""
import itertools,re,sys
import pcbnew
b=pcbnew.LoadBoard(sys.argv[1]); mm=pcbnew.FromMM
pcbnew.ZONE_FILLER(b).Fill(b.Zones())
sys.path.insert(0,__import__('os').path.dirname(__import__('os').path.abspath(__file__))); import domains
ESTOP={n for n in map(str,b.GetNetsByName().keys()) if domains.is_estop(n)}; HV=domains.is_hv
L=pcbnew.F_Cu
NEED={}
def need(a,c):
    """required clearance (internal units) between two nets on F.Cu (0.2 mm default, more by tools/domains.py), 0 on one net"""
    k=(a,c)
    if k not in NEED: v=domains.clearance(a,c,True); NEED[k]=mm(v) if a!=c else 0
    g=NEED[k]
    if MODE=='estop': return mm(1.5) if g>=mm(1.5) else (min(g,mm(0.2)) if BASE else 0)
    return g if (BASE or g>mm(0.2)) else 0
MODE='estop'   # pass 1: only the E-stop 1.5 mm pairs (moves keep 0.2 mm to everything); pass 2: every rule
BASE=False   # True while testing a move: then the 0.2 mm default counts too, so a part never lands on other copper
def copper():
    """F.Cu copper as (net, shape-or-pad, owner footprint, bbox); pad shapes are read live, so moves show up"""
    out=[]
    for p in b.GetPads():
        if p.IsOnLayer(L): out.append((p.GetNetname(),p,p.GetParentFootprint()))
    for t in b.GetTracks():
        if t.IsOnLayer(L) and (t.GetClass()!='PCB_VIA' or t.FlashLayer(L)): out.append((t.GetNetname(),t.GetEffectiveShape(L),None))
    for z in b.Zones():   # GND fills refill round whatever moves, so they're not obstacles
        if z.IsOnLayer(L) and not z.GetIsRuleArea() and z.GetNetname() and z.GetNetname()!='GND':
            fp=z.GetFilledPolysList(L)
            if fp.OutlineCount(): out.append((z.GetNetname(),fp,None))
    return out
def near(cu,fp,r=mm(8)):
    """copper within r of the footprint (moves are at most 6 mm, the rule at most 1.5 mm)"""
    F=fp.GetBoundingBox(); F=(F.GetLeft()-r,F.GetRight()+r,F.GetTop()-r,F.GetBottom()+r)
    out=[]
    for it in cu:
        B=bb(it[1])
        if not (B[1]<F[0] or B[0]>F[1] or B[3]<F[2] or B[2]>F[3]): out.append(it)
    return out
def shape(x): return x.GetEffectiveShape(L) if isinstance(x,pcbnew.PAD) else x
BBC={}
def bb(x):
    """bounding box; tracks and pours don't move, so theirs are cached (BBox() of a filled pour is slow)"""
    if isinstance(x,pcbnew.PAD): r=x.GetBoundingBox(); return r.GetLeft(),r.GetRight(),r.GetTop(),r.GetBottom()
    k=id(x)
    if k not in BBC: r=x.BBox(); BBC[k]=(r.GetLeft(),r.GetRight(),r.GetTop(),r.GetBottom(),x)
    return BBC[k][:4]
def viol(fp,cu):
    """(pad of fp, owner) pairs closer than the spacing rule (tools/domains.py): E-stop 1.5 mm to 54 V, 0.6 mm between
    54 V domains; pairs inside one footprint are the package's"""
    out=[]
    for p in fp.Pads():
        if not p.IsOnLayer(L): continue
        P_=bb(p); s=None
        for n,x,o in cu:
            if o==fp: continue
            g=need(p.GetNetname(),n)
            if not g: continue
            X=bb(x)
            if P_[1]+g<X[0] or X[1]+g<P_[0] or P_[3]+g<X[2] or X[3]+g<P_[2]: continue
            s=s or p.GetEffectiveShape(L)
            if s.Collide(shape(x),g-1): out.append((p,o))
    return out
def crt(fp):
    c=fp.GetCourtyard(pcbnew.F_CrtYd); return c if c.OutlineCount() else None
EDGE=b.GetBoardEdgesBoundingBox()
NEARFP={}
def ok_place(fp):
    c=crt(fp)
    if c is None: return True
    bb=c.BBox()
    if not EDGE.Contains(bb.GetOrigin()) or not EDGE.Contains(bb.GetEnd()): return False
    for g in NEARFP.get(fp.GetReference(),b.GetFootprints()):
        if g==fp: continue
        d=crt(g)
        if d is not None and c.Collide(d.Outline(0),mm(0.1)): return False
    return True
TRKS=list(b.GetTracks())
def wired(fp):
    """parts with power-stage copper on their pads (ties, in-pad vias, the brake pair) stay where they are"""
    F=fp.GetBoundingBox()
    for t in TRKS:
        if not t.GetBoundingBox().Intersects(F): continue
        for p in fp.Pads():
            if any(p.HitTest(q) for q in ((t.GetPosition(),) if t.GetClass()=='PCB_VIA' else (t.GetStart(),t.GetEnd()))): return True
    return False
RMAX=int(3/0.25)   # 3 mm: past that a part leaves its block's routing
STEPS=sorted({(dx,dy) for dx in range(-RMAX,RMAX+1) for dy in range(-RMAX,RMAX+1)},key=lambda d:d[0]**2+d[1]**2)[1:]
NOMOVE={'U1'}   # bank B's hot-swap controller keeps its sense and gate loops; its neighbours move instead
FIXED=('H','TH','HS','J')   # holes (their keep-outs are fixed) and connectors (the harness interface) never move
def try_move(fp,check):
    global BASE
    BASE=True
    try: return _try_move(fp,check)
    finally: BASE=False
import time,os
DEADLINE=0
def _try_move(fp,check):
    if time.time()>DEADLINE: return None
    p0=fp.GetPosition(); F=fp.GetBoundingBox(); r=mm(10)
    NEARFP[fp.GetReference()]=[g for g in b.GetFootprints() if g.GetBoundingBox().Intersects(pcbnew.BOX2I(pcbnew.VECTOR2I(F.GetLeft()-r,F.GetTop()-r),pcbnew.VECTOR2L(F.GetWidth()+2*r,F.GetHeight()+2*r)))]
    for dx,dy in STEPS:
        fp.SetPosition(pcbnew.VECTOR2I(p0.x+mm(0.25*dx),p0.y+mm(0.25*dy)))
        if ok_place(fp) and check(): return (0.25*dx,-0.25*dy)
    fp.SetPosition(p0); return None
moved=[]
def setsteps(rmm):
    global RMAX; RMAX=int(rmm/0.25); STEPS[:]=sorted({(dx,dy) for dx in range(-RMAX,RMAX+1) for dy in range(-RMAX,RMAX+1)},key=lambda d:d[0]**2+d[1]**2)[1:]
# pass 1, E-stop 1.5 mm pairs: up to 6 mm moves, then a last round out to HVR (10 mm), 20 min; pass 2, the 0.6 mm domain pairs: 3 mm moves, 15 min (HVT)
for MODE,radii,budget in (('estop',(4,4,6)+tuple(float(x) for x in os.environ.get('HVR','10').split(',')),1200),('all',(3,3),float(os.environ.get('HVT','900')))):
 DEADLINE=time.time()+budget
 for rmm in radii:
  setsteps(rmm); left=[]; hv=copper()
  for fp in list(b.GetFootprints()):
      loc=near(hv,fp)
      if not viol(fp,loc): continue
      v=viol(fp,loc)
      d=None if wired(fp) or fp.GetReference() in NOMOVE or fp.GetReference().rstrip('0123456789') in FIXED else try_move(fp,lambda: not viol(fp,loc))
      if d: moved.append((fp.GetReference(),d)); continue
      # move the 54 V neighbour(s) instead, if they are free parts (no pad in a pour, not fixed by the power stage)
      others={o.GetReference():o for _,o in v if o is not None}.values()
      done=True
      for o in others:
          if o.IsLocked() or wired(o) or o.GetReference() in NOMOVE or o.GetReference().rstrip('0123456789') in FIXED or any(z.HitTestFilledArea(L,p.GetPosition()) for p in o.Pads() if p.GetNetCode() for z in b.Zones() if z.GetNetCode()==p.GetNetCode()):
              done=False; continue
          lo=near(hv,o)
          def chk(o=o,lo=lo): return not [1 for _,q in viol(fp,loc) if q==o] and not viol(o,lo)
          d=try_move(o,chk)
          if d: moved.append((o.GetReference(),d))
          else: done=False
      if not done or viol(fp,loc): left.append((fp.GetReference(),sorted({p.GetNumber()+':'+p.GetNetname() for p,_ in viol(fp,loc)}),sorted({o.GetReference() if o else 'pour/track/via' for _,o in viol(fp,loc)})))
  hv=copper()
# pours and vias the moves can't clear: keep pours 1.5 mm off each remaining E-stop pad (rule area, no copper pour, F.Cu)
# and drop 54 V vias within 1.5 mm of it (they sit in arrays; the rest of the array carries the current)
pcbnew.ZONE_FILLER(b).Fill(b.Zones())
nko=nvia=0
for fp in b.GetFootprints():
    for p in [p for p in fp.Pads() if p.GetNetname() in ESTOP and p.IsOnLayer(L)]:
        s=p.GetEffectiveShape(L)
        for t in list(b.GetTracks()):
            if t.GetClass()=='PCB_VIA' and HV(t.GetNetname()) and t.GetEffectiveShape(L).Collide(s,mm(1.5)-1): b.Remove(t); nvia+=1
        if any(z.IsOnLayer(L) and HV(z.GetNetname()) and not z.GetIsRuleArea() and z.GetFilledPolysList(L).Collide(s,mm(1.5)-1) for z in b.Zones()):
            pb=p.GetBoundingBox(); g=mm(1.6)
            k=pcbnew.ZONE(b); k.SetIsRuleArea(True); k.SetDoNotAllowZoneFills(True); k.SetDoNotAllowTracks(False); k.SetDoNotAllowVias(False)
            k.SetDoNotAllowPads(False); k.SetDoNotAllowFootprints(False); k.SetLayer(L); k.SetZoneName(f'ESTOP_GUARD_{fp.GetReference()}_{p.GetNumber()}')
            o=k.Outline(); o.NewOutline()
            for x,y in ((pb.GetLeft()-g,pb.GetTop()-g),(pb.GetRight()+g,pb.GetTop()-g),(pb.GetRight()+g,pb.GetBottom()+g),(pb.GetLeft()-g,pb.GetBottom()+g)): o.Append(pcbnew.VECTOR2I(x,y))
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
cu=copper(); left=[(fp.GetReference(),sorted({p.GetNumber()+':'+p.GetNetname()+'/'+(o.GetReference() if o else 'copper') for p,o in viol(fp,cu)})) for fp in b.GetFootprints() if viol(fp,cu)]
pcbnew.ZONE_FILLER(b).Fill(b.Zones())
b.Save(sys.argv[1])
print('moved',len(moved),moved); print('left',len(left))
for r in left: print('  ',r)
