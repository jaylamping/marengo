#!/usr/bin/env python3
"""Absorber loop inductance LABS (and LBAR) from the routed copper, for tools/absorber_sim.py.
Method: each L1 conductor of the loop is a wide microstrip over the solid In1 GND plane, whose loop inductance (trace plus
its image return in the plane) is mu0 * h * l / w, with h = 0.2104 mm (JLC04161H-7628, L1 to In1). The current divides
over the 8 cans (4 finger rows x 2 columns), so each segment counts with (I_seg / I)^2 (energy method). Part ESLs are
datasheet-class figures, listed below as assumptions. Part positions come from the board; conductor widths from the
pours of tools/route_power.py.
Run with KiCad's Python: <kicad python> tools/absorber_loop.py <board.kicad_pcb>"""
import sys
import pcbnew
MU0=4e-7*3.141592653589793; H=0.2104e-3
b=pcbnew.LoadBoard(sys.argv[1])
def pad(r,n):
    p=[q for q in b.FindFootprintByReference(r).Pads() if q.GetNumber()==n][0].GetPosition()
    return pcbnew.ToMM(p.x)-150,340-(pcbnew.ToMM(p.y)-100)
def ms(l,w,frac=1.0): return MU0*H*(l*1e-3)/(w*1e-3)*frac*frac
rows=sorted({round(pad(c,'1')[1],1) for c in ('C38','C44','C45','C82','C83','C84','C85','C86')},reverse=True)   # can + pads
nrow=len(rows); ncan=8
seg=[]
# 1. BAT pour from the pack-entry ceramics to the D7/D21 BAT pads (pour about 10 mm wide there)
cb=pad('C89','1'); d7=pad('D7','2'); seg.append(('BAT pour, ceramics to D7/D21',ms(abs(d7[0]-cb[0])+abs(d7[1]-cb[1]),10.0)))
# 2. D7 || D21: SMC body and leads, about 1.5 nH each (assumed)
seg.append(('D7 || D21 (SMC, 1.5 nH each, assumed)',1.5e-9/2))
# 3. ABS pour from the diode ABS pads to the comb spine (12.5..27.9 x 301.9..306.6)
da=pad('D7','1'); seg.append(('ABS pour, diodes to spine',ms(26.3-(da[0]+pad('D21','1')[0])/2,4.7)))
# 4. ABS spine (2.6 mm) from the pour (Y about 304) to each finger row; I/nrow per row
y0=304.25; up=[r for r in rows if r>y0]; dn=sorted([r for r in rows if r<y0],reverse=True)
for grp in (up,sorted(up),):
    pass
yy=y0
for k,r in enumerate(sorted(up)): seg.append((f'ABS spine up to {r}',ms(r-yy,2.6,(len(up)-k)/nrow))); yy=r
yy=y0
for k,r in enumerate(dn): seg.append((f'ABS spine down to {r}',ms(yy-r,2.6,(len(dn)-k)/nrow))); yy=r
# 5. ABS fingers (4 mm): spine to the first can column (I/nrow), then to the second (I/ncan)
for r in rows: seg+= [(f'ABS finger {r} a',ms(30.8-26.3,4.0,1/nrow)),(f'ABS finger {r} b',ms(42.2-30.8,4.0,1/ncan))]
# 6. cans: EEH-ZC 10 x 10.2 hybrid, about 3 nH ESL each (assumed), I/8 each
seg.append(('8 cans, 3 nH ESL each (assumed)',8*3e-9/ncan**2))
# 7. ABS_N fingers (4 mm) back to the ABS_N spine at X 46.65
for r in rows: seg+= [(f'ABS_N finger {r+8.4:.1f} a',ms(42.2-30.8,4.0,1/ncan)),(f'ABS_N finger {r+8.4:.1f} b',ms(46.65-42.2,4.0,1/nrow))]
# 8. ABS_N spine (2.7 mm) down to R53; it collects one row more at each step
r53=pad('R53','1'); yy=None; nr=sorted([r+8.4 for r in rows],reverse=True)
for k,r in enumerate(nr):
    nxt=nr[k+1] if k+1<len(nr) else r53[1]
    seg.append((f'ABS_N spine {r:.1f} -> {nxt:.1f}',ms(r-nxt,2.7,(k+1)/nrow)))
# 9. R53 copper link and its GND via grid into In1, about 0.2 nH (assumed)
seg.append(('R53 link + GND vias (assumed)',0.2e-9))
LABS=sum(v for _,v in seg)
for n,v in seg: print(f'  {v*1e9:6.3f} nH  {n}')
print(f'LABS = {LABS*1e9:.2f} nH')
# LBAR: absorber node (D7/D21 BAT pads) to the bank A drain: BAT pour to RS1's top pad, RS1 itself, SW_D to the drain ceramics
rs=pad('RS1','1'); c87=pad('C87','1')
lbar=[('BAT pour, D7/D21 to RS1',ms(abs(d7[0]-rs[0])+abs(d7[1]-rs[1]),8.0)),
      ('RS1 CSS4J 4026, about 1 nH (assumed)',1.0e-9),
      ('SW_D pour, RS1 to C87/C88',ms(abs(rs[0]-c87[0])+abs(pad('RS1','4')[1]-c87[1]),6.0))]
for n,v in lbar: print(f'  {v*1e9:6.3f} nH  {n}')
print(f'LBAR = {sum(v for _,v in lbar)*1e9:.2f} nH')
