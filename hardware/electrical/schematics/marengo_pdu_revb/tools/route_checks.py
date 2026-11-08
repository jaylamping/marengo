#!/usr/bin/env python3
"""Stage 3 checks after the signal routing (LAYOUT_RULES_REVB.md), beyond KiCad's DRC:
  1. E-stop safety nets keep >= 1.5 mm to 54 V copper on every layer (the rule exists because 0.44 MOhm of leakage into
     UVLO enables bank B by itself).
  2. 54 V copper keeps >= 0.6 mm (outer) / 0.2 mm (inner) to every net of another voltage domain (IPC-2221B, 51-100 V).
  Net classes and domains: tools/domains.py (the router uses the same table). Pairs inside one footprint are listed
  separately: the package sets those, not the routing.
  3. Kelvin sense pairs: each SH*_P / SH*_N pair runs as a tight pair; reports the length of each leg and the share of
     the P leg that lies within 1.0 mm of the N leg.
Run with KiCad's Python: <kicad python> tools/route_checks.py <board.kicad_pcb>   (exit 1 on a routing violation)"""
import re,sys
import pcbnew
b=pcbnew.LoadBoard(sys.argv[1]); mm=pcbnew.FromMM
sys.path.insert(0,__import__('os').path.dirname(__file__)); import domains
ESTOP={n for n in map(str,b.GetNetsByName().keys()) if domains.is_estop(n)}
HV=domains.is_hv; dom=domains.dom; RIDES=domains.rides
LAYERS=[pcbnew.F_Cu,pcbnew.In1_Cu,pcbnew.In2_Cu,pcbnew.B_Cu]
OUTER={pcbnew.F_Cu,pcbnew.B_Cu}
items=[]   # (net, layer, shape, label, footprint ref or None)
def fpref(it):
    f=it.GetParentFootprint() if hasattr(it,'GetParentFootprint') else None
    return f.GetReference() if f else None
for L in LAYERS:
    for p in b.GetPads():
        if p.IsOnLayer(L):   # an unflashed inner layer of a PTH pad keeps only its hole
            s=p.GetEffectiveShape(L) if p.FlashLayer(L) else p.GetEffectiveHoleShape()
            items.append((p.GetNetname(),L,s,f'{fpref(p)}.{p.GetNumber()}',fpref(p)))
    for t in b.GetTracks():
        if t.IsOnLayer(L):
            v=t.GetClass()=='PCB_VIA'
            s=t.GetEffectiveShape(L) if not v or t.FlashLayer(L) else t.GetEffectiveHoleShape()
            items.append((t.GetNetname(),L,s,'via' if v else 'track',None))
    for z in b.Zones():
        if z.IsOnLayer(L) and not z.GetIsRuleArea() and z.GetNetname():
            fp=z.GetFilledPolysList(L)
            if fp.OutlineCount(): items.append((z.GetNetname(),L,fp,f'zone {z.GetNetname()}',None))
def pos(s):
    bb=s.BBox(); c=bb.GetCenter(); return round(pcbnew.ToMM(c.x)-150,1),round(340-(pcbnew.ToMM(c.y)-100),1)
BB={}
def box(s):
    k=id(s)
    if k not in BB: bb=s.BBox(); BB[k]=(bb.GetLeft(),bb.GetRight(),bb.GetTop(),bb.GetBottom())
    return BB[k]
def collide(a,c,cl):
    A,C=box(a),box(c); g=mm(cl)
    if A[1]+g<C[0] or C[1]+g<A[0] or A[3]+g<C[2] or C[3]+g<A[2]: return False
    try: return a.Collide(c,mm(cl))
    except Exception:
        return c.Collide(a,mm(cl))
bad=[]; pkg=[]
byL={L:[i for i in items if i[1]==L] for L in LAYERS}
for L in LAYERS:
    # every pair of different nets where one side is 54 V (or rides on 54 V) or E-stop: the rule from tools/domains.py
    hot=[i for i in byL[L] if RIDES(i[0]) or i[0] in ESTOP]
    G=5000000   # 5 mm buckets
    bk={}
    for it in byL[L]:
        x0,x1,y0,y1=box(it[2])
        for gx in range(x0//G,x1//G+1):
            for gy in range(y0//G,y1//G+1): bk.setdefault((gx,gy),[]).append(it)
    for m,_,h,lab2,ref2 in hot:
        x0,x1,y0,y1=box(h); r=mm(1.5); cand={}
        for gx in range((x0-r)//G,(x1+r)//G+1):
            for gy in range((y0-r)//G,(y1+r)//G+1):
                for it in bk.get((gx,gy),()): cand[id(it)]=it
        for n,_,s_,lab,ref in cand.values():
            if n==m or ((RIDES(n) or n in ESTOP) and (n,lab)<(m,lab2)): continue   # each hot-hot pair once
            cl=domains.clearance(m,n,L in OUTER)
            if cl<=0.2 or not collide(h,s_,cl-0.001): continue
            rec=(b.GetLayerName(L),m,lab2,n,lab,pos(s_),f'<{cl} mm')
            (pkg if ref and ref==ref2 else bad).append(rec)
print('1/2. clearance violations (routing):',len(bad))
for r in bad: print('   ',r)
print('   inside one footprint (package-set, review only):',len(pkg))
for r in pkg[:20]: print('   ',r)
# ---- Kelvin pairs
tr={}
for t in b.GetTracks():
    if t.GetClass()=='PCB_TRACK': tr.setdefault(t.GetNetname(),[]).append(t)
print('3. Kelvin pairs: leg lengths (mm), share of P within 1.0 mm of N')
for n in sorted(tr):
    m=re.match(r'(/SH\w*)_P$',n)
    if not m or m.group(1)+'_N' not in tr: continue
    P,Nn=tr[n],tr[m.group(1)+'_N']; lp=sum(pcbnew.ToMM(t.GetLength()) for t in P); ln=sum(pcbnew.ToMM(t.GetLength()) for t in Nn)
    near=0.0
    for t in P:
        s=t.GetStart(); e=t.GetEnd(); k=max(1,int(pcbnew.ToMM(t.GetLength())/0.25))
        for i in range(k):
            q=pcbnew.VECTOR2I(int(s.x+(e.x-s.x)*(i+0.5)/k),int(s.y+(e.y-s.y)*(i+0.5)/k))
            if any(u.GetEffectiveShape(u.GetLayer()).Collide(q,mm(1.0)+u.GetWidth()//2) for u in Nn if u.GetLayer()==t.GetLayer()):
                near+=pcbnew.ToMM(t.GetLength())/k
    print(f'   {m.group(1)}: P {lp:.1f}, N {ln:.1f}, paired {100*near/max(lp,1e-9):.0f} %')
sys.exit(1 if bad else 0)
