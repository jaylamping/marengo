#!/usr/bin/env python3
"""Stage 3 checks after the signal routing (LAYOUT_RULES_REVB.md), beyond KiCad's DRC:
  1. E-stop safety nets (ESTOP_SAFE) keep >= 1.5 mm to 54 V copper on every layer (the rule exists because 0.44 MOhm of
     leakage into UVLO enables bank B by itself). Pad-to-pad pairs inside one footprint are listed separately: the
     package sets those, not the routing.
  2. 54 V copper keeps >= 0.6 mm (outer) / 0.2 mm (inner) to every other net (IPC-2221B, 51-100 V).
  3. Kelvin sense pairs: each SH*_P / SH*_N pair runs as a tight pair; reports the length of each leg and the share of
     the P leg that lies within 1.0 mm of the N leg.
Run with KiCad's Python: <kicad python> tools/route_checks.py <board.kicad_pcb>   (exit 1 on a routing violation)"""
import re,sys
import pcbnew
b=pcbnew.LoadBoard(sys.argv[1]); mm=pcbnew.FromMM
ESTOP=set('/'+n for n in 'RUN RUN_A RUN_B RUN_P UVLO EN_A FLT_A FLT_D L_B L_E L_PB L_PC MID_B VB_B INT_B WDO_N VWD'.split())
ESTOP|={n for n in b.GetNetsByName().keys() if re.match(r'/(LOOP_|RST_)',str(n))}
ESTOP={str(n) for n in ESTOP}
HV=lambda n: bool(re.match(r'/(BAT|SW_D|MID|VBUS|ABS|ABS_N|CL_D)$|/(BRK_|SH|V_L|D_L)',n)) and n not in ESTOP and not n.startswith('/SH12')
def dom(n):
    """54 V-riding domain: a port's gate, bootstrap and pull-up/down nets ride on its V_L, its sense and input nets on VBUS;
    pairs inside one domain are tens of volts apart at most, so the 0.6 mm rule doesn't apply to them"""
    m=re.match(r'/(?:V|G|GPD|GPU|BST|GC)_L(\d)$',n)
    if m: return 'VL'+m.group(1)
    if re.match(r'/(?:D_L\d|CSP_L\d|INP_L\d|SHL\d_[PN]|VBUS)$',n): return 'VBUS'
    if re.match(r'/(?:BAT|SW_D|SH_[PN]|SH9_[PN]|BRK_D|CSP_A|CSN_A|VS_A)$',n): return 'BAT'
    return None
# LAYOUT_RULES_REVB.md's list, read literally (BAT, SW_D, MID, VBUS, ABS, CL_D, BRK_*, SH*, V_L*) plus ABS_N and the port
# drains D_L*; the latch's own supply chain (BAT_F, BAT_R, BAT_CTL) and the divider taps are not in it
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
    es=[i for i in byL[L] if i[0] in ESTOP]; hv=[i for i in byL[L] if HV(i[0])]
    for n,_,s,lab,ref in es:
        for m,_,h,lab2,ref2 in hv:
            if collide(s,h,1.5-0.001):
                rec=(b.GetLayerName(L),n,lab,m,lab2,pos(s))
                (pkg if ref and ref==ref2 else bad).append(rec)
    cl=0.6 if L in OUTER else 0.2
    oth=[i for i in byL[L] if not HV(i[0])]
    for m,_,h,lab2,ref2 in hv:
        for n,_,s,lab,ref in oth:
            if n!=m and not (ref and ref==ref2) and not (dom(n) and dom(n)==dom(m)) and collide(h,s,cl-0.001):
                bad.append((b.GetLayerName(L),m,lab2,n,lab,pos(s),f'<{cl} mm'))
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
