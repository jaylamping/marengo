# Manufacturing sheets (this thread) from out/ renders at DPR 2. Run after shoot.js and split_check.py.
import json
exec(open('compose.py').read().split("sub = 'Concept only")[0])
import os; os.makedirs('final', exist_ok=True)
rep = json.load(open('out/split_report.json'))
sub = f"Manufacturing study, concept geometry with a {rep['wall_mm']} mm wall. Not a part. 2026-11-06"
C = (900, 150, 2300, 2250)
sheet([(crop('sp_asm.png', C, w=900), 'Front: seams on the sternum and side'),
       (crop('sp_asm_back.png', C, w=900), 'Back: compute door (blue), battery door (green)'),
       (crop('sp_explode.png', (500, 0, 2700, 2300), w=1150), 'Exploded: service units off the frame'),
       (crop('sp_explode_back.png', (500, 0, 2700, 2300), w=1150), 'Exploded, back')],
      f"Body shell split for the Bambu P2S: {len(rep['pieces'])} prints, {len({q['unit'] for q in rep['pieces']})} service units", sub, 'SHELL_MFG_SPLIT.png',
      notes=['White: chest panels, split L/R on the sternum and spine (print splits, bolted on inner splice plates). Purple: shoulder-top hatches.',
             'Orange: side vent grilles (swappable cooling variants). Pink: shoulder collars, split front/back so they come off with the arm on.',
             'Dark: abdomen bands, front/back halves; TPU spine pack pads on the back. Every piece fits 250 mm usable of the 256 mm cube (SHELL_MFG_PLATES.png).'])
sheet([(crop('sp_mounts.png', C, w=900), 'Hardpoints, front'),
       (crop('sp_mounts_back.png', C, w=900), 'Hardpoints, back'),
       (crop('sp_air.png', C, w=900), 'Cooling: chimney flow')],
      'Mounting hardpoints and airflow', sub, 'SHELL_MFG_MOUNTS.png',
      notes=['Orange blocks: 20 printed PETG hardpoint brackets on M5 T-nuts (top rails, step rail between the gussets, bottom rails, lower posts, abdomen carrier).',
             'Brackets hold the M3 heat-set inserts; panels only carry clearance holes and two locating pins, so a reprinted panel needs no inserts.',
             'Air: in through the front power-board fan, side grilles and band gaps (blue), up past the pack, Pi and Jetson, out through the D1 fans and neck ring (red). Blue box: 13S2P pack.'])
sheet([(crop('sp_plates.png', (0, 0, 3200, 2400), w=2200), 'Each print in its chosen orientation inside a 256 mm P2S volume')],
      f"Print plates: all {len(rep['pieces'])} pieces", sub, 'SHELL_MFG_PLATES.png',
      notes=[f"Shell about {rep['total_part_g']} g PETG, spine pack {rep['spine_pack_g']} g TPU, plus {rep['total_support_g']} g support, {rep['total_hours'][0]} to {rep['total_hours'][1]} h of printing (estimate, not a slice).",
             'Orientation picked by script: fits the bed, then least support under the visible outer face. Tallest print: upper back, 246 mm of 250.'])
SC = (1000, 150, 2200, 1450)
sheet([(crop('fix_side_base.png', SC, w=700), 'Locked baseline: back on the frame'),
       (crop('fix_side_new.png', SC, w=700), 'Fixed: back +10, +20 at the bottom'),
       (crop('fix_iso_base.png', (700, 0, 2500, 2400), w=700), 'Baseline, back 3/4'),
       (crop('fix_iso_new.png', (700, 0, 2500, 2400), w=700), 'Fixed, back 3/4')],
      'Frame clearance fix: the back was too shallow for the 2020 frame', sub, 'SHELL_MFG_BACKFIX.png',
      notes=['Baseline: lower-box rear corners come through the wall by up to 11 mm at the frame bottom; under 1 mm at the gusset corners; under 8 mm across the back.',
             'Fix: back 10 mm deeper (20 mm at the frame bottom), bottom 4 mm wider per side and slightly squarer. Front, shoulders, socket and waist unchanged.',
             'Result: 5.6 mm at the very bottom corners, 7.8 mm at the gusset corners, 9 to 17 mm elsewhere (frame_clearance.py). Chest depth 212 -> 222 mm.'])

sheet([(crop('pk_back.png', (600, 0, 2600, 2400), w=900), 'Back 3/4: pads flank the doors'),
       (crop('pk_back_ortho.png', (900, 100, 2300, 2300), w=760), 'Back'),
       (crop('pk_side.png', (900, 100, 2300, 2300), w=760), 'Side: 30 mm proud of the shell')],
      'Spine pack: back crush pads (first pass, function only; battery door between)', sub, 'SHELL_MFG_SPINEPACK.png',
      notes=['A backward fall needs ~40 mm of stop; these give 30 (more pad or standoff crush to come). TPU 95A, ~10% gyroid; coupon C9 decides.',
             f"Four pads, about {rep['spine_pack_g']} g. They bolt through the shell to the H2/H3/H4 rear brackets on standoff posts; D1 and the D2 battery door open between them.",
             'Styling (armour plates, chamfers) comes in SolidWorks. Lighter option: EVA foam blocks in a thin TPU skin, about 60 g.'])
