#!/usr/bin/env python3
"""Power board package STEP for placing it in the torso (Joey, 2026-11-07): the board with its KiCad 3D models, the option-D
heatsink, placeholder blocks for the connectors that have no KiCad model, and named keep-out bodies for the mating plugs
and cable exits. Coordinates are the board's floorplan coordinates (X -50..50, Y 170..340, the same as the torso
coordinates in the frame docs); Z = 0 is the board's bottom face, components on +Z, heatsink on -Z.
WIRE_* bodies (Joey, 2026-11-07): every conductor leaving the board, swept at its true outer diameter from its exit (pad or
mated-plug face) about 80-120 mm out in its natural direction, at its minimum bend radius; rows that bend the same way get
concentric bends so they nest. Colour by net: red = battery/VBUS +, black = return, orange = brake FET drain, yellow = 12 V,
blue = control, green = E-stop loop, white/violet = Pi signals. The same table is written to harness_exits.csv
(HARNESS_INTERFACE.md); full in-torso routing belongs to the frame thread.
Usage (CadQuery): python tools/package_step.py <board.step from kicad-cli, --user-origin 150x100mm> <conn.json> <out.step> [exits.csv]
conn.json: from tools/conn_export.py (courtyard box and numbered pads of every connector, floorplan coordinates)."""
import csv,json,math,sys
import cadquery as cq
board=cq.importers.importStep(sys.argv[1]).translate((0,340,0))   # kicad-cli puts Y = floorplan Y - 340 at that origin
bb=board.val().BoundingBox()                                          # kicad-cli: board bottom face at Z 0, top at 1.6; THT leads reach Z -1.2
conn=json.load(open(sys.argv[2]))
def box(x0,x1,y0,y1,z0,z1): return cq.Workplane().box(x1-x0,y1-y0,z1-z0,centered=False).translate((x0,y0,z0))
T=1.6
a=cq.Assembly(name='marengo_power_board_revb5')
a.add(board,name='board_with_parts',color=cq.Color(0.1,0.35,0.15))
# connectors without a KiCad 3D model: courtyard footprint, approximate mated height with the plug (Micro-Fit 3.0 about 16 mm,
# Mini-Fit Jr about 22 mm)
for r,h in (('J3',22),('J25',22),('J5',16),('J6',16),('J7',16),('J22',16),('J23',16),('J24',16)):
    x0,x1,y0,y1=conn[r]['box']; a.add(box(x0,x1,y0,y1,T,T+h),name=f'{r}_approx',color=cq.Color(0.2,0.2,0.2))
# heatsink (THERMAL_OPTION.md option D, chosen by Joey 2026-11-07): commodity 120 x 69 x 36 mm extruded aluminium, 27 fins
# along Y, 4.6 mm base, fitted as bought (120 mm; the 100 x 170 board moved the Micro-Fit pins to Y 175, F2's pins start at
# Y 319): X -34.5..34.5, Y 196.5..316.5, on a 0.5 mm insulating thermal pad;
# 2 x M3 through HS1 (0, 286) and HS2 (0, 204) into holes drilled and tapped in the base on its centre fin
HX0,HX1,HY0,HY1,HB,HH=-34.5,34.5,196.5,316.5,4.6,36.0
a.add(box(HX0,HX1,HY0,HY1,-0.5,0),name='heatsink_thermal_pad',color=cq.Color(0.6,0.6,0.9))
hs=box(HX0,HX1,HY0,HY1,-0.5-HB,-0.5)
for k in range(27):
    x=HX0+0.4+k*(69.0-0.8)/26
    hs=hs.union(box(x-0.4,x+0.4,HY0,HY1,-0.5-HH,-0.5-HB))
for X,Y in ((0.0,286.0),(0.0,204.0)): hs=hs.cut(cq.Workplane().circle(1.25).extrude(10).translate((X,Y,-0.5-10)))
a.add(hs,name='heatsink_120x69x36',color=cq.Color(0.75,0.75,0.78))
# keep-outs (translucent red): what must stay free for plugs, cables and air
KO={'KO_pack_8AWG_leads_R15_bend':(-5,25,325,358,-12,22),
    'KO_brake_plugs_J3_J25':(25,41,327.5,365,T,30),
    'KO_12V_converter_plug_J24':(-42,-18,328.5,365,T,30),
    'KO_bottom_plugs_J5_J7_J21_J22_J23':(-38,24,145,180,T,30),
    'KO_estop_plug_J20':(-70,-35,295,311,T,9),
    'KO_limb_pigtails_right':(42,80,203,252,-6,20),
    'KO_limb_pigtails_left':(-80,-42,203,252,-6,20),
    'KO_heatsink_air_inlet':(-35,35,316.5,336.5,-37,-0.5),
    'KO_heatsink_air_outlet':(-35,35,176.5,196.5,-37,-0.5)}
for n,(x0,x1,y0,y1,z0,z1) in KO.items(): a.add(box(x0,x1,y0,y1,z0,z1),name=n,color=cq.Color(0.9,0.1,0.1,0.25))
# ---- wires
COL={'red':(0.8,0.05,0.05),'black':(0.08,0.08,0.08),'orange':(1.0,0.45,0.0),'yellow':(0.95,0.85,0.1),'blue':(0.1,0.3,0.9),
     'green':(0.1,0.65,0.2),'white':(0.92,0.92,0.92),'violet':(0.55,0.2,0.8),'pink':(1.0,0.5,0.7)}
def colour(net):
    n=net.lstrip('/')
    if n in ('BAT','VBUS') or n.startswith('V_L') or n=='BRK_F': return 'red'
    if n=='GND' or n.startswith('D_L'): return 'black'
    if n=='CL_D': return 'orange'
    if n.startswith('+12V') or n.startswith('SPARE'): return 'yellow'
    if n in ('LOOP_OUT','LOOP_RET','ESTOP_NO'): return 'green'
    if n in ('RST_SRC','RST_IN'): return 'violet'
    if n=='+3V3_PI': return 'pink'
    if n=='BRK_ON': return 'blue'
    return 'white'
V=lambda *c: cq.Vector(*c)
EXITS=[]
def wire(name,net,p0,d0,od,rise=0.0,bend=None,R=0.0,run=100.0,note=''):
    """centreline: from p0 along d0 for rise, then (if bend) a 90 deg arc of radius R into direction bend, then run straight"""
    p0=V(*p0); d0=V(*d0).normalized(); e=[]; p=p0
    if rise>0: q=p+d0*rise; e.append(cq.Edge.makeLine(p,q)); p=q
    d=d0
    if bend is not None:
        b=V(*bend).normalized(); c=p+b*R; mid=c-b*R*math.cos(math.pi/4)+d0*R*math.sin(math.pi/4); q=c+d0*R
        e.append(cq.Edge.makeThreePointArc(p,mid,q)); p=q; d=b
    q=p+d*run; e.append(cq.Edge.makeLine(p,q))
    path=cq.Wire.assembleEdges(e)
    prof=cq.Wire.makeCircle(od/2,p0,d0)
    s=cq.Solid.sweep(cq.Face.makeFromWires(prof),path,transitionMode='round')
    a.add(cq.Workplane().add(s),name=name,color=cq.Color(*COL[colour(net)]))
    EXITS.append(dict(body=name,net=net.lstrip('/'),od_mm=od,x=round(p0.x,2),y=round(p0.y,2),z=round(p0.z,2),
                      dx=round(d0.x,3),dy=round(d0.y,3),dz=round(d0.z,3),bend_R_mm=R if bend is not None else '',
                      then_dx=round(d.x,3),then_dy=round(d.y,3),then_dz=round(d.z,3),note=note))
def pads(r): return conn[r]['pads']
def nested(r,ztop,od,Rmin,dirn,run,tag,note):
    """vertical exits from a mated plug top at z=ztop, all bending into dirn (+1: +Y, -1: -Y); concentric bends"""
    P=pads(r); front=max(y*dirn for _,x,y,_n in P)
    for num,x,y,net in P:
        R=Rmin+(front-y*dirn); wire(f'WIRE_{tag}_{r}_{num}_{net.lstrip("/")}',net,(x,y,ztop),(0,0,1),od,0,(0,dirn,0),R,run,note)
# pack pair, 8 AWG fine-strand silicone, 7.0 mm OD, soldered from the top, R15 over the top edge (REV_B_PARTS pack row)
for num,x,y,net in pads('J1'): wire(f'WIRE_PACK_8AWG_J1_{num}_{net.lstrip("/")}',net,(x,y,T),(0,0,1),7.0,3.0,(0,1,0),15.0,90.0,'8 AWG to the XT90 board half, mating the pack XT90S in its case-wall pocket (mating face toward -X)')
# limb pigtails out of the side edge at 3 x OD: arms, waist, aux 16 AWG silicone (3.0 mm OD) into the XT30(2+2), matching
# the arm trunk; legs keep 14 AWG (3.6 mm OD) for rev C. Aux and legs are unfitted on rev B5
LIMB={'J8':'L_ARM','J9':'R_ARM','J10':'WAIST','J11':'AUX','J12':'L_LEG','J13':'R_LEG'}
for r,tag in LIMB.items():
    s=1 if conn[r]['box'][0]>0 else -1; od,g=(3.6,'14') if 'LEG' in tag else (3.0,'16')
    for num,x,y,net in pads(r): wire(f'WIRE_LIMB_{tag}_{r}_{num}_{net.lstrip("/")}',net,(x,y,T),(0,0,1),od,2.0,(s,0,0),3*od,100.0,
                                      f'{g} AWG pigtail to inline 20 A MINI fuse and '+('XT60' if 'LEG' in tag else 'XT30(2+2)-F')+('' if tag in ('L_ARM','R_ARM','WAIST') else ' (unfitted)'))
# brake leads: Mini-Fit Jr plug (mated top about 22 mm above the board), 18 AWG silicone twisted pair, 2.3 mm OD, toward +Y
for r in ('J3','J25'): nested(r,T+22.0,2.3,8.0,1,90.0,'BRAKE','18 AWG twisted pair to the HS50 on the '+('+X' if r=='J3' else '-X')+' gusset')
# 12 V converter pigtail J24: Micro-Fit 3.0 2 x 6 (mated top about 16 mm), 18 AWG thin-wall, 1.9 mm OD, toward +Y
nested('J24',T+16.0,1.9,6.0,1,100.0,'BUCK12','18 AWG, twisted in facing pairs, about 150 mm to the buck board')
# bottom-edge 12 V outputs, Micro-Fit 3.0 (mated top about 16 mm), toward -Y: 18 AWG for Jetson, Pi, spares; 22 AWG for fans
for r,tag,od in (('J5','JETSON12',1.9),('J6','PI12',1.9),('J7','FAN12',1.3),('J22','SPARE_SW',1.9),('J23','SPARE_ON',1.9)):
    nested(r,T+16.0,od,4*od,-1,100.0,tag,{'J7':'22 AWG to the fan splitter'}.get(r,'18 AWG'))
# JST GH 1.25 side-entry cables, 26 AWG, 1.0 mm OD, leaving the plug back (plug about 6.5 mm long), 2.2 mm above the board
for r,tag,ax in (('J21','PI_LINK',(0,-1,0)),('J20','ESTOP_LOOP',(-1,0,0))):
    x0,x1,y0,y1=conn[r]['box']
    for num,x,y,net in pads(r):
        p=(x,y0-6.5,T+2.2) if ax[1] else (x0-6.5,y,T+2.2)
        wire(f'WIRE_{tag}_{r}_{num}_{net.lstrip("/")}',net,p,ax,1.0,0,None,0,90.0,'26 AWG GH cable'+(' to the Pi' if r=='J21' else ' to E-stop, reset, thermostats'))
a.save(sys.argv[3],exportType='STEP')
if len(sys.argv)>4:
    w=csv.DictWriter(open(sys.argv[4],'w',newline='',encoding='utf-8'),fieldnames=list(EXITS[0])); w.writeheader(); w.writerows(EXITS)
print(len(EXITS),'wires')
print('board bbox',round(bb.xlen,1),round(bb.ylen,1),round(bb.zlen,1),'saved',sys.argv[3])
