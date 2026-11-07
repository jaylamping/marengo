#!/usr/bin/env python3
"""Rev B5 power routing, stage 2: copper pours and via arrays for the 60 A path (LAYOUT_RULES_REVB.md, THERMAL_OPTION.md D).
Run with KiCad's Python: <kicad python> tools/route_power.py <placed.kicad_pcb> <out.kicad_pcb>
Coordinates are the floorplan's (X -50..50, robot's left = +X; Y 190..340 up), as in tools/place.py.
L1 (F.Cu): power pours. In1: solid GND. In2: GND (second return plane; signals cut it later). L4 (B.Cu): mirrors of the
power pours under the FETs (heatsink pads) and the VBUS sheet to the ports, GND fill elsewhere.
Prints, for each pour, any pad of another net that lies inside it (to be moved or carved), then fills and saves."""
import sys
import pcbnew
OX,OY=100,100
def P(X,Y): return pcbnew.VECTOR2I(pcbnew.FromMM(OX+X+50),pcbnew.FromMM(OY+340-Y))
b=pcbnew.LoadBoard(sys.argv[1])
NET={n:b.FindNet(n) for n in ('GND','/BAT','/SW_D','/MID','/VBUS','/ABS','/ABS_N','/CL_D')+tuple(f'/{a}_L{k}' for a in 'DV' for k in range(1,7))}
L1,L4,I1,I2=pcbnew.F_Cu,pcbnew.B_Cu,pcbnew.In1_Cu,pcbnew.In2_Cu
POURS=[]
def zone(net,layer,rects,prio,clear=0.6,name=''):
    """one zone whose outline is the union of the rectangles (X0,X1,Y0,Y1); solid pad connection (power), 0.25 mm min width"""
    z=pcbnew.ZONE(b); z.SetLayer(layer); z.SetNet(NET[net]); z.SetAssignedPriority(prio)
    z.SetLocalClearance(pcbnew.FromMM(clear)); z.SetMinThickness(pcbnew.FromMM(0.25))
    z.SetPadConnection(pcbnew.ZONE_CONNECTION_FULL); z.SetIslandRemovalMode(pcbnew.ISLAND_REMOVAL_MODE_ALWAYS)
    o=z.Outline()
    for (x0,x1,y0,y1) in rects:
        o.NewOutline()
        for X,Y in ((x0,y0),(x1,y0),(x1,y1),(x0,y1)): o.Append(P(X,Y))
        POURS.append((net,layer,(x0,x1,y0,y1),name))
    o.Simplify()
    b.Add(z)
VIAS=[]
def vias(net,x0,x1,y0,y1,pitch=1.0,drill=0.3,dia=0.6):
    n=0; X=x0
    while X<=x1+1e-6:
        Y=y0
        while Y<=y1+1e-6:
            v=pcbnew.PCB_VIA(b); v.SetPosition(P(X,Y)); v.SetDrill(pcbnew.FromMM(drill)); v.SetWidth(pcbnew.FromMM(dia))
            v.SetNet(NET[net]); b.Add(v); n+=1; Y+=pitch
        X+=pitch
    VIAS.append((net,n,(x0,x1,y0,y1)))
# ---------------- L1 power pours ----------------
# BAT: pack pair -> RS1's top pad, D7/D21 top pads; arm left to D1 and RS9 (the 12 V converter feed)
zone('/BAT',L1,[(-2.2,25.2,309.7,339.6),(-18.8,-2.2,324.2,334.2)],5,name='BAT')
# SW_D: RS1's bottom pad -> bank A tabs (strip above the pins) and the drain ceramics C87/C88
zone('/SW_D',L1,[(-16.2,11.2,300.9,303.6),(-1.9,3.6,300.9,304.6),(-14.8,-6.9,300.9,306.8)],5,name='SW_D')
# MID: bank A sources -> bank B tabs, the 8.6 mm band between the banks; the pins 2/3 islands go through vias to L4
zone('/MID',L1,[(-17.3,18.0,281.9,290.0),(-17.3,-12.3,290.0,295.4),(0.7,5.7,290.0,295.4)],5,name='MID')
zone('/MID',L1,[(-18.4,-12.3,296.9,299.0),(0.7,5.7,296.9,299.0)],5,name='MID pins 2-3')
# VBUS: bank B sources -> the + pads of the five bus caps and D4; pins 2/3 islands via L4
zone('/VBUS',L1,[(-28.8,28.8,262.6,267.8),(-17.3,17.6,267.8,270.2),(-17.3,-12.3,270.2,275.4),(0.7,5.7,270.2,275.4),(17.6,32.4,266.2,267.8)],5,name='VBUS caps')
zone('/VBUS',L1,[(-18.4,-12.3,276.9,279.0),(0.7,5.7,276.9,279.0)],5,name='VBUS pins 2-3')
# ABS and ABS_N combs over the can stack, D7/D21 bottom pads to the cans, ABS_N down to R53
zone('/ABS',L1,[(12.5,27.9,301.9,306.6)]+[(25.0,44.2,y-2.0,y+2.0) for y in (316.5,302.7,288.9,275.1)]+[(25.0,27.6,273.1,318.5)],6,name='ABS')
zone('/ABS_N',L1,[(28.8,48.0,y-2.0,y+2.0) for y in (324.9,311.1,297.3,283.5)]+[(45.3,48.0,271.9,326.9),(42.4,46.0,269.2,273.4)],7,name='ABS_N')
# ports: VBUS stub at the shunt's bus pad, D_L shunt -> tab, V_L pins 5-7 -> output clamp -> pigtail. Right ports as
# drawn; the left ports' power row is turned 180 degrees in tools/place.py, so their rects turn the same way.
PORTS=[(1,253.5,1),(2,253.5,-1),(3,236.0,1),(4,236.0,-1),(5,218.5,1),(6,218.5,-1)]   # 4-6 unfitted (aux, legs) but routed
def turn(s,Y0,Y1,r):
    x0,x1,y0,y1=r
    return r if s>0 else (-x1,-x0,Y0+Y1-y1,Y0+Y1-y0)
for k,Y1,s in PORTS:
    Y0=Y1-17.5; T=lambda r:turn(s,Y0,Y1,r)
    zone('/VBUS',L1,[T((6.8,9.4,Y1-7.6,Y1-0.2)),T((6.8,12.8,Y1-2.56,Y1-0.46))],5,name=f'VBUS L{k}')
    zone(f'/D_L{k}',L1,[T((16.4,20.7,Y1-3.83,Y1-1.73))],5,name=f'D_L{k}')
    zone(f'/V_L{k}',L1,[T((26.6,47.3,Y0+0.1,Y0+5.3)),T((43.0,47.3,Y0+5.3,Y0+7.3))],5,name=f'V_L{k}')
# ---------------- inner GND planes ----------------
zone('GND',I1,[(-49.6,49.6,190.4,339.6)],0,clear=0.5,name='GND In1')
zone('GND',I2,[(-49.6,49.6,190.4,339.6)],0,clear=0.5,name='GND In2')
# ---------------- L4: mirrors under the FETs (heatsink pads), VBUS sheet, GND fill ----------------
zone('/SW_D',L4,[(-11.0,-0.3,290.2,303.6),(7.0,17.8,290.2,303.6),(-11.0,17.8,300.2,304.6)],5,name='SW_D L4')   # bridged above the pins
zone('/MID',L4,[(-18.4,18.0,270.2,290.0),(-18.4,-12.3,290.0,299.4),(0.7,5.7,290.0,299.4)],4,name='MID L4')
zone('/VBUS',L4,[(-28.8,28.8,262.0,270.0),(-18.4,-12.3,270.0,279.4),(0.7,5.7,270.0,279.4)],5,name='VBUS L4 bank')
zone('/VBUS',L4,[(-12.8,12.8,200.0,262.0)],3,name='VBUS L4 to ports')
for k,Y1,s in PORTS:
    Y0=Y1-17.5; T=lambda r:turn(s,Y0,Y1,r)
    zone(f'/D_L{k}',L4,[T((19.8,32.0,Y0+6.9,Y1-0.1))],5,name=f'D_L{k} L4')
    zone(f'/V_L{k}',L4,[T((21.5,47.3,Y0+0.1,Y0+5.6))],5,name=f'V_L{k} L4')
zone('GND',L4,[(-49.6,49.6,190.4,339.6)],0,clear=0.5,name='GND L4')
zone('GND',L1,[(-49.6,49.6,190.4,339.6)],0,clear=0.5,name='GND L1 fill')
# ---------------- via arrays ----------------
vias('/SW_D',-9.8,-1.6,291.0,300.6); vias('/SW_D',8.0,16.6,291.0,300.6)          # under the bank A tabs
vias('/SW_D',-1.4,3.1,301.4,303.4,pitch=0.9)                                     # under RS1's SW_D pad neck, to the L4 bridge
vias('/MID',-9.8,-1.6,271.0,280.6); vias('/MID',8.0,16.6,271.0,280.6)            # under the bank B tabs
vias('/MID',-16.0,-13.0,282.6,289.4); vias('/MID',2.8,5.8,282.6,289.4)           # band between the banks (clear of HS1 at 0,286)
vias('/MID',-18.0,-17.6,297.27,298.54,pitch=1.27); vias('/MID',1.2,1.2,297.27,298.54,pitch=1.27)   # pins 2-3 islands
vias('/VBUS',-16.9,16.9,267.0,269.6,pitch=1.3); vias('/VBUS',-26.0,-18.0,266.9,266.9,pitch=1.0); vias('/VBUS',18.0,26.0,266.9,266.9,pitch=1.0)   # bank B sources and bus caps to L4
vias('/VBUS',-18.0,-17.6,277.27,278.54,pitch=1.27); vias('/VBUS',1.2,1.2,277.27,278.54,pitch=1.27)
for k,Y1,s in PORTS:
    Y0=Y1-17.5; T=lambda r:turn(s,Y0,Y1,r)
    for r,n in (((7.3,9.3,Y1-7.0,Y1-1.0),'/VBUS'),((21.3,30.5,Y0+8.2,Y1-1.5),f'/D_L{k}'),((31.0,32.0,Y0+0.6,Y0+4.6),f'/V_L{k}')):
        x0,x1,y0,y1=T(r); vias(n,x0,x1,y0,y1)                                      # VBUS field, under the tab, V_L to L4
# ---------------- tracks ----------------
def track(net,layer,pts,w):
    for (x0,y0),(x1,y1) in zip(pts,pts[1:]):
        s=pcbnew.PCB_TRACK(b); s.SetStart(P(x0,y0)); s.SetEnd(P(x1,y1)); s.SetWidth(pcbnew.FromMM(w)); s.SetLayer(layer)
        s.SetNet(NET[net] if net in NET else b.FindNet(net)); b.Add(s)
def C(v): return (pcbnew.ToMM(v.x)-OX-50,340-(pcbnew.ToMM(v.y)-OY))
# D2PAK-7: the pin 4 lead and the tab are one drain but two pads; join them under the body (0.5 mm keeps 0.6 mm to pins 3/5)
for fp in b.GetFootprints():
    if 'TO-263-7' not in fp.GetFPIDAsString(): continue
    p4=[p for p in fp.Pads() if p.GetNumber()=='4']
    lead=min(p4,key=lambda p:p.GetSize(pcbnew.F_Cu).x*p.GetSize(pcbnew.F_Cu).y); tab=max(p4,key=lambda p:p.GetSize(pcbnew.F_Cu).x*p.GetSize(pcbnew.F_Cu).y)
    if lead.GetNetname(): track(lead.GetNetname(),L1,[C(lead.GetPosition()),C(tab.GetPosition())],0.5)
# port FETs: source pins 2-3 (beside the gate) reach the L4 V_L mirror through a via at each pin's outer end
for k,ref in ((1,'Q15'),(2,'Q16'),(3,'Q17'),(4,'Q18'),(5,'Q19'),(6,'Q20')):
    fp=b.FindFootprintByReference(ref)
    for p in fp.Pads():
        if p.GetNumber() in ('2','3'):
            X,Y=C(p.GetPosition()); end=-1.6 if X>0 else 1.6   # outer end: down on the right ports, up on the turned left ports
            v=pcbnew.PCB_VIA(b); v.SetPosition(P(X,Y+end)); v.SetDrill(pcbnew.FromMM(0.3)); v.SetWidth(pcbnew.FromMM(0.6)); v.SetNet(p.GetNet()); b.Add(v)
# brake headers J3/J25 at the top edge: VBUS and CL_D up the can column on L4, 2 mm (6 A for both resistors)
for X,Y,n in ((31.5,266.9,'/VBUS'),(37.5,271.0,'/CL_D')):
    v=pcbnew.PCB_VIA(b); v.SetPosition(P(X,Y)); v.SetDrill(pcbnew.FromMM(0.5)); v.SetWidth(pcbnew.FromMM(1.0)); v.SetNet(NET[n]); b.Add(v)
track('/VBUS',L4,[(31.5,266.9),(40.5,266.9),(40.5,337.15),(29.0,337.15)],2.0)
track('/CL_D',L4,[(37.5,271.0),(34.5,274.0),(34.5,329.0),(37.0,331.65)],2.0); track('/CL_D',L4,[(34.5,329.0),(29.0,331.65)],2.0)
track('/CL_D',L1,[(37.47,268.48),(37.5,271.0)],2.0)
track('/CL_D',L1,[(32.0,262.26),(34.0,263.6),(34.0,266.0),(36.2,268.2),(37.47,268.48)],1.2)   # round C49
v=pcbnew.PCB_VIA(b); v.SetPosition(P(36.0,265.4)); v.SetDrill(pcbnew.FromMM(0.4)); v.SetWidth(pcbnew.FromMM(0.8)); v.SetNet(NET['/VBUS']); b.Add(v)
track('/VBUS',L1,[(36.0,263.19),(36.0,265.4)],1.0); track('/VBUS',L4,[(36.0,265.4),(36.0,266.9)],1.5)   # C49 to the L4 brake feed
# power GND pads straight down to the In1/In2 planes: a via grid inside each pad (0.3 mm drill, tented, small enough
# not to starve the joint on the hot plate)
GNDREFS='C10 C11 C12 C13 C47 C49 C87 C88 C89 C90 C91 C92 D1 D25 D28 D31 D34 D37 D40 R53 C14 C15'.split()
for r in GNDREFS:
    fp=b.FindFootprintByReference(r)
    if not fp: continue
    for p in fp.Pads():
        if p.GetNetname()!='GND': continue
        bb=p.GetBoundingBox(); x0,y1=C(pcbnew.VECTOR2I(bb.GetX(),bb.GetY())); x1,y0=C(pcbnew.VECTOR2I(bb.GetRight(),bb.GetBottom()))
        nx=max(1,int((x1-x0-0.6)/1.0)+1); ny=max(1,int((y1-y0-0.6)/1.0)+1)
        for i in range(nx):
            for j in range(ny):
                X=(x0+x1)/2+(i-(nx-1)/2)*1.0; Y=(y0+y1)/2+(j-(ny-1)/2)*1.0
                v=pcbnew.PCB_VIA(b); v.SetPosition(P(X,Y)); v.SetDrill(pcbnew.FromMM(0.3)); v.SetWidth(pcbnew.FromMM(0.6)); v.SetNet(NET['GND']); b.Add(v)
# ---------------- report pads of other nets inside each pour ----------------
pads=[(fp.GetReference(),p) for fp in b.GetFootprints() for p in fp.Pads()]
bad={}
for net,layer,(x0,x1,y0,y1),name in POURS:
    if net=='GND' or layer!=L1: continue
    for r,p in pads:
        X,Y=C(p.GetPosition())
        if x0<X<x1 and y0<Y<y1 and p.IsOnLayer(layer) and p.GetNetname() not in (net,''):
            bad.setdefault(name,set()).add(f'{r}.{p.GetNumber()}={p.GetNetname()}')
for k,v in bad.items(): print('foreign pads in',k,':',' '.join(sorted(v)))
print('vias:',sum(n for _,n,_ in VIAS))
pcbnew.ZONE_FILLER(b).Fill(b.Zones())
b.Save(sys.argv[2]); print('saved',sys.argv[2])
