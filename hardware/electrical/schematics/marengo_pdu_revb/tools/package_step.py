#!/usr/bin/env python3
"""Power board package STEP for placing it in the torso (Joey, 2026-11-07): the board with its KiCad 3D models, the option-D
heatsink, placeholder blocks for the connectors that have no KiCad model, and named keep-out bodies for the mating plugs
and cable exits. Coordinates are the board's floorplan coordinates (X -50..50, Y 190..340, the same as the torso
coordinates in the frame docs); Z = 0 is the board's bottom face, components on +Z, heatsink on -Z.
Usage (CadQuery): python tools/package_step.py <board.step from kicad-cli, --user-origin 150x100mm> <conn.json> <out.step>
conn.json: connector courtyard boxes {ref: [x0, x1, y0, y1]} in floorplan coordinates."""
import json,sys
import cadquery as cq
board=cq.importers.importStep(sys.argv[1]).translate((0,340,0))   # kicad-cli puts Y = floorplan Y - 340 at that origin
bb=board.val().BoundingBox()                                          # kicad-cli: board bottom face at Z 0, top at 1.6; THT leads reach Z -1.2
conn=json.load(open(sys.argv[2]))
def box(x0,x1,y0,y1,z0,z1): return cq.Workplane().box(x1-x0,y1-y0,z1-z0,centered=False).translate((x0,y0,z0))
T=1.6
a=cq.Assembly(name='marengo_power_board_revb5')
a.add(board,name='board_with_parts',color=cq.Color(0.1,0.35,0.15))
# connectors without a KiCad 3D model: courtyard footprint, approximate height (Micro-Fit 3.0 vertical about 10 mm,
# Mini-Fit Jr vertical about 13 mm)
for r,h in (('J3',13),('J25',13),('J5',10),('J6',10),('J7',10),('J22',10),('J23',10),('J24',10)):
    x0,x1,y0,y1=conn[r]; a.add(box(x0,x1,y0,y1,T,T+h),name=f'{r}_approx',color=cq.Color(0.2,0.2,0.2))
# heatsink (THERMAL_OPTION.md option D): insulating gap pad, base, fins along Y
a.add(box(-35,35,200,305,-0.5,0),name='heatsink_gap_pad',color=cq.Color(0.6,0.6,0.9))
hs=box(-35,35,200,305,-6.0,-0.5)
for k in range(12): hs=hs.union(box(-34.25+k*6.0,-32.75+k*6.0,200,305,-25.0,-6.0))
a.add(hs,name='heatsink_70x105x25',color=cq.Color(0.75,0.75,0.78))
# keep-outs (translucent red): what must stay free for plugs, cables and air
KO={'KO_pack_8AWG_leads_R15_bend':(-5,25,325,358,-12,22),
    'KO_brake_plugs_J3_J25':(25,41,327.5,365,T,30),
    'KO_12V_converter_plug_J24':(-42,-18,328.5,365,T,30),
    'KO_bottom_plugs_J5_J7_J21_J22_J23':(-38,24,165,200,T,30),
    'KO_estop_plug_J20':(-70,-35,295,311,T,9),
    'KO_limb_pigtails_right':(42,80,205,252,-6,14),
    'KO_limb_pigtails_left':(-80,-42,205,252,-6,14),
    'KO_heatsink_air_inlet':(-35,35,305,325,-25,-0.5),
    'KO_heatsink_air_outlet':(-35,35,180,200,-25,-0.5)}
for n,(x0,x1,y0,y1,z0,z1) in KO.items(): a.add(box(x0,x1,y0,y1,z0,z1),name=n,color=cq.Color(0.9,0.1,0.1,0.25))
a.save(sys.argv[3],exportType='STEP')
print('board bbox',round(bb.xlen,1),round(bb.ylen,1),round(bb.zlen,1),'saved',sys.argv[3])
