#!/usr/bin/env python3
"""Stage 3 input for Freerouting: export the Specctra DSN and make it safe to autoroute around the power copper.
Freerouting reads every (plane) and every polygon wire as a non-obstacle conduction area, so other nets would be routed
straight through the power pours. This script therefore:
  - removes every plane, then adds GND back on In1 only and declares In1 a power layer (no signals on it; GND pins
    get vias to it);
  - drops the GND fills on L1, In2 and L4 (they are refilled around the signals after import);
  - exports every other filled zone (power pours, +12V_RAW strip) as fixed traces of its own net: scan lines 0.5 mm wide
    at 0.4 mm pitch inside the fill. They are obstacles to other nets, and their own net's pins can still connect to them;
  - sets the net classes: POWER (54 V copper) 0.6 mm clearance (IPC-2221B external, 51-100 V), P12 1.0 mm tracks,
    everything else 0.2/0.2. Existing tracks and vias stay fixed.
Run with KiCad's Python: <kicad python> tools/dsn_prep.py <board.kicad_pcb> <out.dsn>"""
import re,sys
import pcbnew
POWER=['/BAT','/SW_D','/MID','/VBUS','/ABS','/ABS_N','/CL_D','/BRK_D','/BRK_F','/BAT_F','/PWR']+[f'/{a}_L{k}' for a in 'DV' for k in range(1,7)]
P12=['/+12V_RAW','/+12V','/+12V_JET','/+12V_PI','/SPARE_SW','/SPARE_ON','/SP_SW1']
W,PITCH=0.5,0.4
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
    """poly: list of closed rings (outline + holes) in mm; even-odd scanline segments"""
    ys=[y for r in poly for _,y in r]; y=min(ys)+W/2; segs=[]
    while y<=max(ys)-W/2+1e-9:
        xs=[]
        for r in poly:
            for (x1,y1),(x2,y2) in zip(r,r[1:]+r[:1]):
                if (y1<=y<y2) or (y2<=y<y1): xs.append(x1+(y-y1)*(x2-x1)/(y2-y1))
        xs.sort()
        for a,c in zip(xs[0::2],xs[1::2]):
            if c-a>W+1e-6: segs.append((a+W/2,c-W/2,y))
        y+=PITCH
    return segs
LN={pcbnew.F_Cu:'F.Cu',pcbnew.In1_Cu:'In1.Cu',pcbnew.In2_Cu:'In2.Cu',pcbnew.B_Cu:'B.Cu'}
wires=[]; nseg={}
for z in b.Zones():
    n=z.GetNetname()
    if not n or n=='GND' or z.GetIsRuleArea(): continue
    for L in z.GetLayerSet().Seq():
        if L not in LN: continue
        fp=z.GetFilledPolysList(L)
        for i in range(fp.OutlineCount()):
            rings=[]
            for k in range(-1,fp.HoleCount(i)):
                c=fp.Outline(i) if k<0 else fp.Hole(i,k)
                rings.append([(pcbnew.ToMM(c.CPoint(j).x),pcbnew.ToMM(c.CPoint(j).y)) for j in range(c.PointCount())])
            for x0,x1,y in scan(rings):
                wires.append(f'    (wire (path {LN[L]} {round(W*1000)}  {round(x0*1000)} {round(-y*1000)}  {round(x1*1000)} {round(-y*1000)})(net {q(n)})(type fix))')
                nseg[n]=nseg.get(n,0)+1
# ---- planes: remove all, put GND back on In1 only
def drop_scopes(text,head):
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
        i=k+1
s=drop_scopes(s,'(plane ')
bb=b.GetBoardEdgesBoundingBox(); c=0.4
x0,x1,y0,y1=um(bb.GetLeft())+400,um(bb.GetRight())-400,-um(bb.GetBottom())+400,-um(bb.GetTop())-400
s=s.replace('    (via "Via[0-3]',f'    (plane GND (polygon In1.Cu 0  {x0} {y0}  {x1} {y0}  {x1} {y1}  {x0} {y1}  {x0} {y0}))\n    (via "Via[0-3]',1)
s=re.sub(r'(\(layer In1\.Cu\s*\(type )signal\)',r'\1power)',s)
# ---- classes
m=re.search(r'\(class kicad_default(.*?)\(circuit',s,re.S)
names=re.findall(r'"[^"]*"|[^\s"]+',m.group(1))
rest=[n for n in names if n.strip('"') not in POWER+P12]
def cls(name,nets,w,cl): return f'    (class {name} {" ".join(nets)}\n      (circuit\n        (use_via "Via[0-3]_600:300_um")\n      )\n      (rule\n        (width {w})\n        (clearance {cl})\n      )\n    )\n'
start=s.rfind('    (class kicad_default',0,m.end()); end=s.find('\n    )\n',m.end())+len('\n    )\n')
s=s[:start]+cls('kicad_default',rest,200,200)+cls('POWER',[q(n) for n in POWER if b.FindNet(n)],600,600)+cls('P12',[q(n) for n in P12 if b.FindNet(n)],1000,300)+s[end:]
# ---- append the hatch to the wiring
k=s.rfind('  (wiring'); e=s.find('\n  )\n',k)
s=s[:e]+'\n'+'\n'.join(wires)+s[e:]
open(sys.argv[2],'w',encoding='utf-8').write(s)
print('dsn',sys.argv[2],'hatch segments',sum(nseg.values()),{k:v for k,v in sorted(nseg.items())})
