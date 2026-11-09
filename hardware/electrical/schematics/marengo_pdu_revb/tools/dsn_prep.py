#!/usr/bin/env python3
"""Stage 3 input for Freerouting: export the Specctra DSN and make it safe to autoroute around the power copper.
Freerouting reads every (plane) and every polygon wire as a non-obstacle conduction area, so other nets would be routed
straight through the power pours. This script therefore:
  - removes every plane; In1 stays solid GND because no class may route on it, and GND is not autorouted (see below);
  - drops the GND fills on L1, In2 and L4 (they are refilled around the signals after import);
  - exports every other filled zone (power pours, +12V_RAW strip) as fixed traces of its own net: scan lines 0.5 mm wide
    at 0.4 mm pitch inside the fill. They are obstacles to other nets, and their own net's pins can still connect to them;
  - sets the net classes: POWER (54 V copper) 0.35 mm clearance (PWRCL; Freerouting applies class clearance between
    same-net vias too, so 0.6 mm turns every 1 mm via array into violations; the 0.6 mm outer-layer rule is checked
    after import by tools/route_checks.py), P12 1.0 mm tracks,
    everything else 0.2/0.2. Existing tracks and vias stay fixed.
Run with KiCad's Python: <kicad python> tools/dsn_prep.py <board.kicad_pcb> <out.dsn>"""
import json,os,re,sys
import pcbnew
sys.path.insert(0,os.path.dirname(os.path.abspath(__file__))); import domains
POWER=['/BAT','/SW_D','/MID','/VBUS','/ABS','/ABS_N','/CL_D','/BRK_D','/BRK_F','/BAT_F','/PWR']+[f'/{a}_L{k}' for a in 'DV' for k in range(1,7)]
P12=['/+12V_RAW','/+12V','/+12V_JET','/+12V_PI','/SPARE_SW','/SPARE_ON','/SP_SW1']
W,PITCH=0.5,0.4
INSET=0.35   # hatch stays this far inside the fill edge, so Freerouting's class clearances (0.6 mm to the full via pad) don't
             # see fake violations against foreign vias the fill only clears by hole + 0.5 mm
b=pcbnew.LoadBoard(sys.argv[1])
pcbnew.ZONE_FILLER(b).Fill(b.Zones())
for t in b.GetTracks(): t.SetLocked(True)
tmp=sys.argv[2]+'.raw'
assert pcbnew.ExportSpecctraDSN(b,tmp)
s=open(tmp,encoding='utf-8').read()
def um(v): return round(pcbnew.ToMM(v)*1000)
def q(n): return f'"{n}"' if re.search(r'[\s()"{}~]',n) else n
# ---- hatch the non-GND fills
def scan(poly):
    """poly: list of closed rings (outline + holes) in mm, already eroded by INSET + W/2; even-odd scanline centrelines"""
    ys=[y for r in poly for _,y in r]; y=min(ys); segs=[]
    while y<=max(ys)+1e-9:
        xs=[]
        for r in poly:
            for (x1,y1),(x2,y2) in zip(r,r[1:]+r[:1]):
                if (y1<=y<y2) or (y2<=y<y1): xs.append(x1+(y-y1)*(x2-x1)/(y2-y1))
        xs.sort()
        for a,c in zip(xs[0::2],xs[1::2]):
            if c-a>0.05: segs.append((a,c,y))
        y+=PITCH
    return segs
LN={pcbnew.F_Cu:'F.Cu',pcbnew.In1_Cu:'In1.Cu',pcbnew.In2_Cu:'In2.Cu',pcbnew.B_Cu:'B.Cu'}
wires=[]; nseg={}; HATCH=[]
for z in b.Zones():
    n=z.GetNetname()
    if not n or n=='GND' or z.GetIsRuleArea(): continue
    for L in z.GetLayerSet().Seq():
        if L not in LN: continue
        fp=pcbnew.SHAPE_POLY_SET(z.GetFilledPolysList(L)); fp.Unfracture()
        fp.Inflate(-pcbnew.FromMM(INSET+W/2),pcbnew.CORNER_STRATEGY_ROUND_ALL_CORNERS,pcbnew.FromMM(0.01))   # erode: trace edges stay INSET inside
        for i in range(fp.OutlineCount()):
            rings=[]
            for k in range(-1,fp.HoleCount(i)):
                c=fp.Outline(i) if k<0 else fp.Hole(i,k)
                rings.append([(pcbnew.ToMM(c.CPoint(j).x),pcbnew.ToMM(c.CPoint(j).y)) for j in range(c.PointCount())])
            for x0,x1,y in scan(rings):
                wires.append(f'    (wire (path {LN[L]} {round(W*1000)}  {round(x0*1000)} {round(-y*1000)}  {round(x1*1000)} {round(-y*1000)})(net {q(n)})(type fix))')
                nseg[n]=nseg.get(n,0)+1; HATCH.append([LN[L],n,round(x0*1000),round(y*1000),round(x1*1000)])
# ---- planes: the power pours stay as planes, run with --router.plane_as_obstacle=true so they block other nets and their
# own net connects to them. The GND fills go: a GND plane on In1 stops other nets' vias through it (fanout escaped 43 % of
# the pins with it, 90 % without), and a class use_layer that leaves out In1 drops every via type that crosses it. In1 is
# priced out in <dsn>.rules instead (marking it inactive blocks every via through it too); it stays (nearly) solid GND. GND is class GNDC: tools/gnd_fanout.py has already dropped a
# via at almost every GND pad, and Freerouting connects the rest (fine-pitch IC pins). The L1/In2/L4 GND fills are refilled
# around the signals after import. HATCH=1 rebuilds the earlier fixed-trace stand-ins for comparison.
HATCHMODE=os.environ.get('HATCH')=='1'
def drop_scopes(text,head,keep=lambda scope: False):
    out=[]; i=0
    while True:
        j=text.find(head,i)
        if j<0: out.append(text[i:]); return ''.join(out)
        out.append(text[i:j]); d=0; k=j
        while True:
            if text[k]=='(': d+=1
            elif text[k]==')':
                d-=1
                if d==0: break
            k+=1
        if keep(text[j:k+1]): out.append(text[j:k+1])
        i=k+1
s=drop_scopes(s,'(plane ',keep=lambda sc: (not HATCHMODE) and not sc.startswith('(plane GND'))
# rule areas that only stop pour fills (the E-stop guards from tools/hv_spacing.py) export as full keep-outs: drop them
s=re.sub(r'\n    \(keepout "" \(polygon [^)]*\)\)','',s)   # structure-level keep-outs only; footprint-image ones are indented deeper
# ---- classes
m=re.search(r'\(class kicad_default(.*?)\(circuit',s,re.S)
names=re.findall(r'"[^"]*"|[^\s"]+',m.group(1))
ESTN=[n for n in names if domains.is_estop(n.strip('"'))]
HVN=[n for n in names if domains.is_hv(n.strip('"')) and n.strip('"') not in POWER+P12]
rest=[n for n in names if n.strip('"') not in POWER+P12+['GND'] and n not in ESTN+HVN]
def cls(name,nets,w,cl): return f'    (class {name} {" ".join(nets)}\n      (circuit\n        (use_via "Via[0-3]_600:300_um")\n      )\n      (rule\n        (width {w})\n        (clearance {cl})\n      )\n    )\n'
start=s.rfind('    (class kicad_default',0,m.end()); end=s.find('\n    )\n',m.end())+len('\n    )\n')
# coated (MARENGO_COATED=1, tools/domains.py): 0.2 mm between 54 V domains; E-stop nets keep 1.5 mm to 54 V copper
# through class-to-class rules (route_checks.py re-checks it after import)
HVW=250; PCL=int(os.environ.get('PWRCL','200' if domains.COATED else '350'))
cc=('    (class_class (classes ESTOPC HV) (rule (clearance 1500)))\n'
    '    (class_class (classes ESTOPC POWER) (rule (clearance 1500)))\n')
s=s[:start]+cls('kicad_default',rest,200,200)+cls('ESTOPC',ESTN,200,200)+cls('HV',HVN,HVW,PCL)+cls('POWER',[q(n) for n in POWER if b.FindNet(n)],600,PCL)+cls('P12',[q(n) for n in P12 if b.FindNet(n)],1000,300)+cls('GNDC',['GND'],300,200)+cc+s[end:]
# ---- append the hatch to the wiring
k=s.rfind('  (wiring'); e=s.find('\n  )\n',k)
if HATCHMODE: s=s[:e]+'\n'+'\n'.join(wires)+s[e:]
# In1: active (an inactive layer also blocks every via through it) but 50x the trace cost, so the router keeps off it;
# tools/ses_import.py reports anything left on In1
open(os.path.splitext(sys.argv[2])[0]+'.rules','w').write('(rules PCB pdu\n  (autoroute_settings\n    (layer_rule In1.Cu (active on) (preferred_direction_trace_costs 50.0) (against_preferred_direction_trace_costs 50.0))\n  )\n)\n')
open(sys.argv[2],'w',encoding='utf-8').write(s)
json.dump(HATCH if HATCHMODE else [],open(sys.argv[2]+'.hatch.json','w'))   # tools/ses_import.py deletes exactly these after the import
print('dsn',sys.argv[2],'hatch segments',sum(nseg.values()),{k:v for k,v in sorted(nseg.items())})
