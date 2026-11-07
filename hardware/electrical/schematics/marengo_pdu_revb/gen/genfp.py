"""Rev B project footprints. Run: python3 -I gen/genfp.py marengo_revb.pretty"""
import sys,uuid
OUT=sys.argv[1]; NS=uuid.UUID('b1d0f6a2-58a7-4a55-9e8e-2d4b6c7e1f03')
BW,BH=57.9,36.8
# Mean Well IQB150W8-SPEC 2026-06-23 p8, top view. Pin rows 50.8 apart, recommended holes 1.4 and 1.9, keep-in 58.4 x 37.3.
PADS=[('1',-25.4,-7.62,1.4),('2',-25.4,0,1.4),('3',-25.4,7.62,1.4),('8',25.4,-7.62,1.9),('7',25.4,-3.81,1.4),('6',25.4,0,1.4),('5',25.4,3.81,1.4),('4',25.4,7.62,1.9)]
HOLES=[(-23.6,-13.1),(23.6,13.1)]  # M3 inserts, 47.2 x 26.2, 3.5 mm per the drawing
def u(k): return str(uuid.uuid5(NS,k))
def rect(x0,y0,x1,y1,l,w): return f'\t(fp_rect (start {x0} {y0}) (end {x1} {y1}) (stroke (width {w}) (type solid)) (fill no) (layer "{l}") (uuid "{u(l)}"))\n'
t=f'''(footprint "IQB150W8"
\t(version 20241229)
\t(generator "marengo_revb_genfp")
\t(layer "F.Cu")
\t(descr "Mean Well IQB150W8 quarter brick. Pins, holes and inserts from IQB150W8-SPEC 2026-06-23 page 8, body 57.9 x 36.8 x 12.7, pin length 4.1.")
\t(tags "dc-dc quarter brick")
\t(attr through_hole)
\t(property "Reference" "REF**" (at 0 -20.5 0) (layer "F.SilkS") (uuid "{u('r')}") (effects (font (size 1 1) (thickness 0.15))))
\t(property "Value" "IQB150W8" (at 0 20.5 0) (layer "F.Fab") (uuid "{u('v')}") (effects (font (size 1 1) (thickness 0.15))))
\t(property "Footprint" "" (at 0 0 0) (layer "F.Fab") (hide yes) (uuid "{u('f')}") (effects (font (size 1 1))))
\t(property "Datasheet" "https://www.meanwell.com/Upload/PDF/IQB150W8/IQB150W8-SPEC.PDF" (at 0 0 0) (layer "F.Fab") (hide yes) (uuid "{u('d')}") (effects (font (size 1 1))))
\t(property "Description" "" (at 0 0 0) (layer "F.Fab") (hide yes) (uuid "{u('e')}") (effects (font (size 1 1))))
'''
t+=rect(-BW/2,-BH/2,BW/2,BH/2,'F.Fab',0.1)+rect(-BW/2-.11,-BH/2-.11,BW/2+.11,BH/2+.11,'F.SilkS',0.12)+rect(-29.45,-18.65,29.45,18.65,'F.CrtYd',0.05)
for num,x,y,d in PADS:
    w=d+1.0
    sh='rect' if num=='1' else 'circle'
    t+=f'\t(pad "{num}" thru_hole {sh} (at {x} {y}) (size {w} {w}) (drill {d}) (layers "*.Cu" "*.Mask") (uuid "{u("p"+num)}"))\n'
for k,(x,y) in enumerate(HOLES):
    t+=f'\t(pad "" np_thru_hole circle (at {x} {y}) (size 3.5 3.5) (drill 3.5) (layers "*.Cu" "*.Mask") (uuid "{u("h"+str(k))}"))\n'
t+='\t(embedded_fonts no)\n)\n'
open(OUT+'/IQB150W8.kicad_mod','w',newline=chr(10)).write(t)

# Pack pigtail pair (rev B5, Joey: the source-to-board run is the heaviest in the robot): 8 AWG fine-strand silicone,
# about 4.0 mm over the strands, so 4.5 mm holes and 8 mm pads at 14 mm pitch; pin 1 BAT (square), pin 2 GND.
D,OD,PIT='4.5','8.0',14.0
t=f'''(footprint "SolderWire-8AWG_1x02_P14mm_D4.5mm_OD8mm"
\t(version 20241229)
\t(generator "marengo_revb_genfp")
\t(layer "F.Cu")
\t(descr "Two soldered 8 AWG wires (pack pigtail), 4.5 mm holes, 8 mm pads, 14 mm pitch")
\t(tags "solder wire 8AWG")
\t(attr through_hole)
\t(property "Reference" "REF**" (at 7 -5.5 0) (layer "F.SilkS") (uuid "{u('pr')}") (effects (font (size 1 1) (thickness 0.15))))
\t(property "Value" "8AWG pair" (at 7 5.5 0) (layer "F.Fab") (uuid "{u('pv')}") (effects (font (size 1 1) (thickness 0.15))))
\t(property "Footprint" "" (at 0 0 0) (layer "F.Fab") (hide yes) (uuid "{u('pf')}") (effects (font (size 1 1))))
\t(property "Datasheet" "" (at 0 0 0) (layer "F.Fab") (hide yes) (uuid "{u('pd')}") (effects (font (size 1 1))))
\t(property "Description" "" (at 0 0 0) (layer "F.Fab") (hide yes) (uuid "{u('pe')}") (effects (font (size 1 1))))
'''
t+=rect(-4.5,-4.5,PIT+4.5,4.5,'F.CrtYd',0.05)
t+=f'\t(pad "1" thru_hole rect (at 0 0) (size {OD} {OD}) (drill {D}) (layers "*.Cu" "*.Mask") (uuid "{u("pp1")}"))\n'
t+=f'\t(pad "2" thru_hole circle (at {PIT} 0) (size {OD} {OD}) (drill {D}) (layers "*.Cu" "*.Mask") (uuid "{u("pp2")}"))\n'
t+='\t(embedded_fonts no)\n)\n'
open(OUT+'/SolderWire-8AWG_1x02_P14mm_D4.5mm_OD8mm.kicad_mod','w',newline=chr(10)).write(t)
