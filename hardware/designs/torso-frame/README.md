# Internal torso frame, rev A (release spec)

2026-11-06. Thread "Finalize the internal torso frame". One frame that the arm thread (`../torso-stepped/`, the FEA and study
record), the shell concept (`../body-shell/concept/`) and the shell manufacturing plan (`../body-shell/manufacturing/`) all agree
on. **Status: spec for Joey's inspection, not yet cut.** Gate review before any aluminium is cut (section 10).

Coordinates: torso STEP frame, mm. **X lateral (robot left +), Y up from the pelvis bottom, Z forward.** Robot frame: robot X =
torso Z, robot Y = torso X, robot Z = torso Y + 810.

| Sheet | Shows |
| --- | --- |
| `FRAME_EXPLODED.png` | Exploded along the frame axis, every part numbered to the table (section 3) |
| `FRAME_EXPLODED_DETAIL.png` | Same, with every screw, T-nut and hardpoint bracket, plus the step-corner joint detail; fastener table |
| `FRAME_ASSEMBLED.png` | Assembled, front-left and back-right |
| `FRAME_DIMENSIONS.png` | Front, side, top with dimensions |
| `FRAME_INSIDE.png` | Everything inside the frame, exploded, frame ghosted |
| `FRAME_PACKAGING.png` | 2D section drawings: internal packaging check (section 6) |
| `FRAME_HARDPOINTS.png` | Shell hardpoint brackets H1-H6, spine pads and collars |
| `FRAME_CHANGES.png` | Built frame vs rev A at the same scale |
| `frame_viewer.html` | Interactive: orbit, explode slider, hover for part names (opens from disk, needs internet for three.js) |

Evidence labels: **recorded CAD check** (arm thread, master torso 022da44), **script check** (ran here), **estimate**, **assumption**, **unresolved**.

## 1. Geometry

Two-box 2020 frame, 6063-T5 T-slot, 6 mm slots. Same 340 mm height as the built frame.

| Item | Value |
| --- | --- |
| Frame bottom / top | Y 155 / 495 |
| Lower box outer | 150 wide (X ±75) × 140 deep (Z ±70), Y 155 to 353.62 |
| Shared step rails | Y 353.62 to 373.62, 226 long, front and rear |
| Upper box outer | 226 wide (X ±113) × 140 deep, Y 373.62 to 495 |
| Step | 38 per side at Y 353.62 |
| Inner clear | lower 110 × 100, upper 186 × 100 |
| Upper bay between the RS03 rear faces | 156.8 wide × 100 deep × 101.4 tall (Y 373.62 to 475) |
| Shoulder pitch axis | Y 424.86 (269.86 above the frame bottom), Z 0 |
| Pitch RS03 rear / housing / output face | X ±78.4 / ±132.5 / **±135** (fixed, arm interface) |
| Shoulder joint line (arm thread) | X ±208.49 |
| Shell collar | 8 unused outboard M4 holes on each pitch RS03 (fixed, arm interface) |
| Gusset plates | 4 × 3 mm 6061-T6 on the Z ±70 faces, Y 285 to 425, X 55 to 113 (and mirror) |
| Hip interface | Bottom rectangle 150 × 140 at Y 155; waist adapter below; waist bay Y 115 to 155 |

## 2. Cut list (script check: `generator/frame.py`)

| Item | Member | Qty | Length, mm | Source |
| --- | --- | --- | --- | --- |
| 1 | Post, lower box, 198.6 | 4 | 198.62 | cut from the existing 340 mm posts |
| 2 | Rail, bottom front/rear, 110 | 2 | 110.00 | existing, reused as is |
| 3 | Rail, bottom side, 100 | 2 | 100.00 | existing, reused as is |
| 4 | Rail, shared step, front/rear, 226 | 2 | 226.00 | **new stock** |
| 5 | Rail, step side, 100 | 2 | 100.00 | existing, reused as is |
| 6 | Rail, shelf, 100 | 2 | 100.00 | **new stock** |
| 7 | Post, upper box, 121.4 | 4 | 121.38 | cut from the existing 340 mm posts |
| 8 | Rail, top front/rear, 186 | 2 | 186.00 | **new stock** |
| 9 | Rail, top side, 100 | 2 | 100.00 | existing, reused as is |

Total extrusion 3124 mm, of which 1024 mm is new stock. 20 corner brackets and 8 end-tapped joints.

The four built 340 mm posts become 198.62 + 121.38, two cuts each, the upper post measured to 121.38 (the 20 mm remainder absorbs the
kerfs). New stock: 2 × 226, 2 × 186, 2 × 100 = 1024 mm; one more 4 × 500 mm pack (`B08Y8KCJW4`) covers it, and the two spare 110 mm
top rails can become the two 100 mm shelf rails instead (then 824 mm new). Spare after the rebuild: four post offcuts.

**Joint count:** 20 corner brackets with 2 × M5 T-nuts each (bottom corners 8, lower post tops 4, upper post bottoms 4,
top corners 4), plus **8 end-tapped joints** at shelf level. 16 are 2028 (the built box's 16, reused); 4 are 2020 (20 mm legs) at the top
inner-X corners, because a 2028 there overlaps the pitch bracket wing at Y 447 to 450. Gate 2 changes: the four top Z-face brackets sat
inside the as-built pitch bracket (arm thread check 11:47Z) and are deleted, the pitch bracket carries that corner (confirm on the built robot);
and (Joey 16:39Z) the 8 shelf rail and step side rail ends are **end-tapped** instead of bracketed: tap M5 into the rail-end core, then an
M5 × 35 socket head goes through the gusset (3 mm) and the shared rail (20 mm) into it, at gusset holes X ±65 and ±103, Y 363.62. That frees
the shelf plate and its cable slots, and the screw doubles as an end anchor for the step joint along the slot.

**Fasteners (all on `FRAME_EXPLODED_DETAIL.png`):**

| Fastener | Qty | For |
| --- | --- | --- |
| M5 × 10 button head + M5 T-nut | 40 | corner brackets |
| M5 × 10 button head + M5 T-nut | 28 | gussets, 7 per plate (the other 2 holes per plate are end-tap screws) |
| M5 × 35 socket head, into an M5-tapped rail end | 8 | end-tapped shelf rail and step side rail ends, through the gussets |
| M5 × 18 button head + M5 T-nut | 20 | shell hardpoint brackets (through 12 mm of PETG; gate 2: × 20 bottoms in the slot before it clamps, × 16 leaves ~2 mm of thread; measure the slot depth first) |
| M5 × 10 button head + M5 T-nut | 8 | pitch bracket ears (4 mm) to the step side rail, top side rail and upper posts (gate 2: × 12 bottoms in the slot; **assumption**: the built bracket's pattern decides) |
| RS03 rear mounting screws | 2 sets | vendor pattern, as built |
| M4 × 10 button head | 16 | shell collars on the RS03 outboard holes |
| M2.5 standoffs and screws | 8 + 4 | Pi 5 stack, Jetson carrier |
| M3 heat-set inserts + M3 × 8 | ~40 | shell panels to the hardpoint brackets (shell thread) |

Total: **96 M5 screws with 96 M5 T-nuts, plus 8 M5 × 35 end-tap screws** on the frame (the pack cradle adds 8 more M5 × 10 + drop-in T-nuts: **104** with the cradle), plus 8 DIN 125 M5 washers under the pitch screws (the ear slots are 6.2 to 6.4 wide, arm thread).

## 2a. Shopping list

What the BOM (`hardware/bom/master-bom.csv`) says Joey bought against what rev A needs. **Confirm the stock on hand before ordering.**

| Item | Bought (BOM) | Needed for rev A | Buy |
| --- | --- | --- | --- |
| 2020 extrusion | the built frame + **an entire spare frame** (Joey 16:44Z) | 1024 mm new (2 × 226, 2 × 186, 2 × 100) | **buy none**: cut the spare frame's four 340 posts into 2 × (226 + 114) and 2 × (186 + 154); trim the two 114s to the 100 shelf rails (square ends: the shelf-level rail ends get tapped). The spare's brackets, T-nuts and screws also count against the lines below |
| 2028 corner brackets | kit of 30, 16 used | 16 | the built box's 16: **buy none** (14 spare) |
| 2020 corner brackets, 20 mm legs | none | 4 | **1 pack of 10** (any 2020-series inside corner bracket, 20 × 20) |
| M5 tap (spiral point or plug) + tap handle, M5 × 35 socket head | none | 1 tap, 8 screws | **1 M5 × 0.8 tap and a 25-pack of M5 × 35 SHCS** for the 8 end-tapped joints (the 2020 core hole is already the 4.2 mm tap drill) |
| M5 T-nuts, 2020, 10 mm long | ~32 with the kit | 96 (+ spares) | **100-pack slide-in** for the corner brackets, plus **two 50-packs of drop-in** (about 56 needed: 28 gussets, 8 pitch brackets, 20 hardpoints; one 50-pack is short) for the slots that are closed once the posts clamp the rail ends (gate review; gate 2 found one 50-pack about 20 short) |
| M5 × 10 button head, stainless | some | 76 | **100-pack** |
| M5 × 18 button head | none | 20 | **25-pack**. Gate 2: × 20 bottoms in the T-nut/slot before the 12 mm PETG bracket is clamped, × 16 leaves ~2 mm of thread. The pitch bracket screws are now M5 × 10 (from the 100-pack, so buy a second 25-pack of × 10). **Measure the slot depth of the stock on hand before ordering** |
| 3 mm 6061-T6 sheet | none | 4 plates 58 × 140 | **one 150 × 300 mm sheet**, or 4 plates laser-cut from `gusset_plate_left.dxf` in this folder (R15, generated by `generator/gusset_dxf.py`; mirror two). The older `../torso-stepped/gusset_plate_left.dxf` is R8 everywhere and is superseded |
| M4 × 10 button head | arm stock | 16 | from the arm fastener stock |
| M4 × 16 thumbscrew with shoulder, M3 heat-set inserts | none | 2 thumbscrews, 4 inserts | pack cradle (`cradle/README.md`); tighten the thumbscrews snug only, each pulls the pack toward its own web |
| DIN 125 M5 washers | none | 8 | **pack of 50** (pitch bracket ears, slots 6.2 to 6.4 wide) |
| M3 heat-set inserts (M3 × 5.7) + M3 × 8 | none | ~40 each | **50-pack each** (shell thread) |
| PETG, TPU 95A | printer stock | ~0.5 kg brackets and adapters, ~0.35 kg pads | top up as needed |
| 40 × 40 × 10 fans, 5 V or 12 V | none | 4 | **4** (2 intake, 2 exhaust) once the PDB voltage rails are fixed |
| Jetson Orin Nano dev kit, M2.5 standoffs | Joey's list | 1 | **confirm whether already owned** |

No new actuators, no new PDB parts yet (board not designed), no new Pi parts.

## 3. Parts, numbered as on `FRAME_EXPLODED.png`

| # | Part | Qty | Material | Mass, g (estimate) | Status |
| --- | --- | --- | --- | --- | --- |
| 1 | Post, lower box, 198.6 | 4 | 2020 | 396 | cut from the built posts |
| 2 | Rail, bottom front/rear, 110 | 2 | 2020 | 110 | reused |
| 3 | Rail, bottom side, 100 | 2 | 2020 | 100 | reused |
| 4 | Rail, shared step, 226 | 2 | 2020 | 226 | **new** |
| 5 | Rail, step side, 100 (the built shelf side rails) | 2 | 2020 | 100 | reused, moved out 38 |
| 6 | Rail, shelf, 100 | 2 | 2020 | 100 | **new** |
| 7 | Post, upper box, 121.4 | 4 | 2020 | 244 | cut from the built posts |
| 8 | Rail, top front/rear, 186 | 2 | 2020 | 186 | **new** |
| 9 | Rail, top side, 100 | 2 | 2020 | 100 | reused, moved out 38 |
| 10 | Gusset plate (`gusset_plate_left.dxf`, R15 at the X 113 / Y 335 corner) | 4 | 3 mm 6061-T6 | 160 | **new**; 40 g each |
| 11 | Corner bracket 2028 + 2 M5 T-nuts | 16 | aluminium | 288 | reused |
| 34 | Corner bracket 2020 (20 mm legs), top inner-X corners | 4 | aluminium | 56 | **new** |
| 12 | RS03 shoulder pitch | 2 | RobStride | 1760 | existing; Ø80 rear boss X 78.4 to 92 with two connector flats, Ø99 body to 132.5, output face 135 |
| 13 | Shoulder pitch bracket | 2 | PETG | 240 | existing part, moved out 38; envelope X 78.4 to 113, Y 353.6 to 495, Z ±50, wing to ±69 at Y 400 to 450 (recorded CAD check) |
| 14 | Compute shelf plate | 1 | existing | 60 | existing; X ±55 between the shelf rails, Y 354.6 to 372.6 (recorded CAD check) |
| 15 | Pi 5 + 2-CH CAN HAT, BNO085 | 1 | | 115 | existing; Y 376 to 397, X −37.5 to 19.7, ports to Z −71.8 (recorded CAD check) |
| 16 | Power + logic boards on a printed carrier (6 × 6 mm M3 F-F standoffs), 0.5 mm insulating sheet, 50 W brake resistor | 1 set | PCB / PETG / Nomex | 355 | board thread plan main 0b6a6ef; carrier Z 37.5 to 41.5 (\|X\| ≤ 55, screwed to the front ends of the tray rails), PCBs from Z 47.5, component face Z 61.5 |
| 17 | RS03 waist yaw | 1 | RobStride | 880 | existing; 57 tall in a 40 bay (**unresolved**, hip stage) |
| 18 | Waist yaw adapter plate | 1 | PETG | 120 | existing; bolt pattern **unresolved** |
| 19–24 | Shell hardpoint brackets H1–H6, PETG with M3 inserts on M5 T-nuts | 20 | PETG | 160 | shell thread; H6 moved (section 5) |
| 25 | Shell collar halves on the RS03 outboard M4 holes | 2 pairs | PETG | 30 | shell part |
| 26 | TPU spine pads (2 upper, 2 lower), 30 mm proud of the shell back, on H2/H3/H4 standoffs | 4 | TPU 95A | 260 | shell thread, main 761c62b (bridges removed for the D2 battery door) |
| 27 | Harness trunks: shoulder runs (stop at \|X\| 52, cross behind the Jetson at Z −58), two side-band trunks (\|X\| 50 to 55, Z 42 to 50), pack path, proposed shelf corner notches | 7 | 12–14 AWG + CAN pair | 200 | routing proposal; the lower-to-upper passage is **open** (section 6) |
| 28 | Pelvis ring | 1 | draft | | hip stage, shown only in the viewer |
| 29 | Jetson Orin Nano dev kit envelope 103 × 90.5 × 45 including its fan | 1 | proposed | 250 | second shelf, Y 410 to 455, ports to the rear; 20 mm to the top rails |
| 30 | Fans 40 × 40 × 10: 2 exhaust, 2 intakes lower front (Y 264 to 304, Y 193 to 233) | 4 | proposed | 80 | exhaust position PROVISIONAL: in the D1 door plane behind the frame face (Y 430 to 470, Z −70 to −80); inside the bay they hit the top rail. The power-board rework will redo the cooling layout |
| 31 | Third compute / USB hub board envelope 90 × 60 × 20 | 1 | proposed | 60 | **no home** (the pack and boards fill the lower box): pelvis, or a layer over the Pi |
| 32 | 13S2P 21700 pack 95 × 162 × 79 + on-pack JK BMS, XT90S, XT60 | 1 | custom pack | 2420 | **locked** 06:13Z; lower box, slides out through D2 (section 6) |
| 33 | Sensors: IMU on the shelf; head cameras/mics future on the top rails | | | | |

Frame structure only (items 1 to 11, 34 to 36: extrusion, gussets, brackets, their screws and T-nuts): about **2.8 kg** (generator sum; gate 2 found the old 2.1 kg figure stale). Everything shown in the torso, pack included, waist and pelvis excluded: about **8.8 kg**.

### Interference check against the real CAD (arm thread, 2026-11-06 04:46Z)

Pitch-bracket fixing (from the original torso STEP, arm thread 06:09Z): 185 g per bracket; the RS03 hangs on 8 M4 SHCS counterbored
from the inboard face (Y 379.6/406.1/443.6/470.1, Z ±18.8/±45.3); the bracket bolts to the frame through four slots in 4 mm ears at
X ±89 to ±93, bolts along X onto the X ±93 member faces: step side rail (Y 363.69, Z 0), top side rail (Y 485, Z 0), front and rear
upper posts (slot Y 416.6 to 432.8, Z ±60). Bolt size is not in the CAD (M5 or M6), read it off the built robot. Note the original
assembly's instance names are swapped against the part files (instance bracket_right-1 is the file bracket_left, at +X).

Jetson (26.9 mm per side), fans, Pi, IMU, PDB envelope and shelf all clear the real RS03s and pitch brackets. Fixed here: the four
top inner-X corner brackets became 2020 (20 mm legs). **Unresolved:** which rear-boss connector flat each RS03 has (A high rear,
Y 447 to 456, Z −31 to −23, or B low front, Y 394 to 402, Z 23 to 31; the real flat differs between left and right, Joey to look).
If A, keep X ±50 to 78, Y 440 to 460, Z −45 to −15 clear (2 mm above the Jetson top); if B, keep shelf supports out of
X ±50 to 78, Y 385 to 410, Z 15 to 40. Plugs have 11 to 12 mm to the bracket, so the harness needs a right-angle or loose-wire
exit going inboard past X ±78.4; a 35 mm bend radius does not fit.

## 4. Arm interfaces (fixed, from the arm thread)

Pitch output face X ±135, shoulder axis Y 424.86, joint line X ±208.49, 8 outboard M4 collar holes free, pitch brackets on the inner
X faces of the upper posts (X ±93) with the RS03 rear face at ±78.4, the same 14.6 mm stand-off as built. The frame does not change
the inward roll limit (15°); that is between the arm and the shell.

## 5. Shell interfaces (agreed with the shell manufacturing thread, 2026-11-06)

| Point | On (torso coords) | Qty | Carries | Status |
| --- | --- | --- | --- | --- |
| H1 | Top front rail, front face, X ±60, Y 485 | 2 | U1 top | free |
| H2 | Top rear rail, rear face, X ±85, Y 485 (shell main 0bfa66a: at ±60 the boss sat on the D1 door edge) | 2 | U2 top, D1 catch, spine pads | free; its T-nut shares the top rear rail slot with the corner-bracket nuts (nut map) |
| H3 | Shared rail front and rear faces, X ±35, Y 364 | 4 | U1/U2 bottom, U3/U4 top, D1 frame; the two rear brackets also carry the **spine rail lip** (\|X\| ≤ 52, Y 355 to 372, 12 mm proud; gate 2: ±60 hit the rear gussets) that the D2 battery door closes onto with magnets | free; **rear brackets stay below Y 374** (Pi ports); the lip tops out at 372 |
| H4 | Bottom rails front and rear faces, X ±40, Y 165 | 4 | U3/U4 bottom, D2 battery door bottom hinge, spine pads | free |
| H5 | Lower post outer side faces, X ±75, Y 255, Z ±60 | 4 | U3/U4 sides, V1 grilles | free (gusset starts at Y 285) |
| H6 | **Moved:** bottom side rail outer faces, X ±75, Y 165, Z ±35 | 4 | abdomen band carrier | agreed; the under-face stays free for the hip interface |
| C | Pitch RS03 outboard M4 holes | 2 | collars | free |

Gussets are not mounts. Doors (shell thread, main 761c62b): D1 at |X| < 62, Y 374 to 492 (rear); D2 is the battery door at |X| ≤ 60, Y 175 to 352 (rear), bottom hinge on H4,
magnets onto the H3 spine rail lip, M3 lock screw for runs. The four spine pads (30 mm TPU, Z −77 to −124) flank the doors: upper |X| 68 to 147, Y 368 to 481; lower |X| 73 to 129 (shell rev B, 193 waist, main c38d83e),
Y 166 to 295; the bridges are gone (the door is there now). They give the 30 to 40 mm of rear crush the tip-over FEA needs (`../torso-stepped/README.md`).

## 6. Internal packaging check (Joey's list, 2026-11-06)

**Power (Joey 2026-11-06 06:13Z): a custom 13S2P 21700 pack in the lower box, frame geometry unchanged.** Study:
`hardware/electrical/power/custom-21700-pack.md`. Pack box 95 × 162 × 79 (26 Molicel P45B/P50B in a 4 × 7 holder grid, cells along Z),
2.2 kg, 46.8 V nominal, 54.6 V full, 421 to 468 Wh. It sits at |X| ≤ 47.5, Y 183 to 345, Z −45 to +34 on two printed tray rails on the
printed **pack cradle** (Joey 16:32Z; SolidWorks model in `cradle/`, main f9621274: webs |X| 48 to 51.5 with 0.5 mm slide clearance per side, feet bolted to the bottom side rails and under the shelf rails, ledge, front lip, top strap, one M4 thumbscrew per side into a pack side insert at Y 252.6), 7.5 mm to the posts, 8 mm to the bottom and shared rails, 25 mm inside the frame rear face, and slides out the
back through D2 (now the battery door, |X| ≤ 60, Y 175 to 352: shell thread, main 761c62b). The **power and bus board** is **one 100 × 150 board (rev B, board thread main 924bab4; the BMS moved onto the pack)** on a
printed carrier (`carrier/`, 118 × 162 × 4 PETG, X ±59, Y 184 to 346), 6 × M3 standoffs at X ±46,
Y 194 / 265 / 336 with 6 mm keep-out pads. (Superseded: the rev A two-PCB layout, power ~100 × 95 over logic ~100 × 55.) The carrier rear face is at Z 37.5 (3 mm air plus a 0.5 mm Nomex sheet over the pack lid at
Z 34); the carrier (Z 37.5 to 41.5, |X| ≤ 59) screws with 4 × M3 × 8 countersunk into heat-set inserts in the cradle's front-edge pads (|X| 51.75, Y 200 and 330), the PCBs sit on 6 mm M3 F-F standoffs from Z 47.5, and the component face is at Z 61.5, 8.5 mm behind the frame front face (the front rails are only at Y 155 to 175 and 353.6 to 373.6; between them the board looks out between the posts). **Component height ceiling (rev B board, one 100 × 150 board, board thread `hardware/electrical/pdb/design/FLOORPLAN_REVB.png`):** parts may rise to **Z 67** (3 mm under the frame front face at Z 70), except in the two intake fan footprints (|X| ≤ 20, Y 264 to 304 and Y 193 to 233; fan bodies Z 70 to 80 in the shell grille), where the ceiling is **Z 61.5** to keep the fan inlet air gap. Accepted exception: the 12 V brick (Y 302.85 to 339.65, top Z 61.8) overlaps the upper fan footprint by 1.15 mm in Y and 0.3 mm in height, leaving 8.2 mm to the fan; no standoff change. The rev A cap board under the pack and the (±46, 239) standoff are dropped. Cooling per the board floorplan rev A (`hardware/electrical/pdb/design/FLOORPLAN.png`): the heat is in the BMS FET banks (Y 294 to 318) and
E-stop banks (Y 266 to 290) on the power board and the 12/24 V DC-DC (Y 192 to 235, left half) on the logic board, so the single intake at
Y 250 to 290 becomes **two 40 mm intakes** in the front panel between the posts, Y 264 to 304 (dropped 8 mm clear of the U1/U3 shell seam at Y 315) and Y 193 to 233 (shell thread: grille
change in U3). The brake resistor is an Arcol HS25, 10 Ω (board thread 11:30Z: 51.0 over the tags, 28.0 × 14.6, holes 18.3 × 19.8; fine for the arm stage, the leg stage takes a bigger part). It lies flat on a 2 mm aluminium adapter plate on the inner face of the **front bottom rail** (|X| ≤ 25.5, Y 154 to 182, Z 33.4 to 50), 1 mm clear of the pack bottom, behind the logic board's bottom edge and in the lower intake's air. The plate hangs 1 mm below the frame bottom (hip stage: keep Y 154 to 155 free at |X| ≤ 35). The HS50 (29.7 wide) overlapped the pack by 1.7 mm. Pack heat (battery thread df11714): about 20 W steady in a hard arm session, 36 W worst sustained (20 A); 40 A is a 10 s burst under 1 K. Lower-box exhaust goes sideways to the V1 side grilles (shell c73d348) through the Z 37.5 to 50 window between the tray rails and the front posts.

**Connector room around the board (checked 2026-11-06 09:2x against the floorplan):** the board envelope is boxed in on three sides. Above the
power board's top edge (Y 340) there are 13.6 mm to the shared front rail (Y 353.6, Z 50 to 70) and the compute shelf plate (Y 354.6, Z ±50,
X ±55). Beside the side edges (X ±50) the front posts (|X| 55 to 75, Z 50 to 70) block Z ≥ 50 and the pack tray rails (|X| 47.5 to 55,
Z −50 to 37.5) and the carrier (to Z 41.5) block the rest, so only an 8 mm band at Z 42 to 50 is open; no plug fits there, only soldered pigtails. Forward there are
8.5 mm to the frame front plane and the fans. Below the logic board's bottom edge (Y 190) there are 35 mm to the waist adapter plate
(Y 155) now that the brake resistor has moved. Resolution (board thread, design README section 1a, main 9ce67a0): **no high-current plug on either board.** High-current exits are
ring-lug pigtails on low-profile M4 board terminals. Reserved cable paths in the generator (item 27): pack power and sense from the HP
top edge over the pack's top edge (X −5 to 25 through the strap gap, Y 345 to 353.6) to the XT90S at the pack's rear top corner, so pack
removal is unplug-then-slide at the D2 door; arm pigtails up the side bands (|X| 50 to 55, Z 42 to 50, Y 160 to 353.6; 5 × 8 mm per side, tight); waist, aux, leg and charge pigtails down the same bands
to inline connectors below Y 190. **Gate 2 correction:** the earlier front-corner path (Z 62 to 70) ran through the front rails and 8 corner brackets, and the
"Z 50 to 60 gap" between the shelf and the shared front rail does not exist (the shelf ends at Z 50, the rail starts there). **There is no cable passage
from the lower box to the upper bay** today. Proposed: a 10 × 10 notch in each front corner of the printed shelf plate (|X| 45 to 55, Z 40 to 50), Joey's call. Only small
connectors on the LP bottom edge mate downward. The shelf plate stays as built (cutout idea withdrawn). Frame bonding: one point, at a carrier standoff, 1 MΩ ∥ 10 nF to system ground (board thread's
proposal, agreed: the frame touches the actuator housings anyway, so "floating" would not be). Connector edges: top = pack XT90 and
16-pin balance (rear face), both arm ports, USB to the Pi, fans; bottom = waist and leg ports, brake resistor; sides = E-stop loop,
charge port, sensors. On-pack BMS (Joey's pick, battery thread main d42e88f): a JK-BD6A17S8P, 133 × 81 × 18 with standoffs, sits on the pack's rear face at X −40.5 to 40.5, Y 186 to 319, Z −65 to −45, long axis along Y, FET heatsink facing rearward toward D2, 5 mm inside the frame rear face. The XT60 charge pigtail parks flat beside the XT90S (X −42 to −17, Y 325 to 345, Z −58 to −45). The sense board and its 2 × 10 connector are gone. All of it stays in |X| ≤ 45, Z ≥ −68 and slides out with the pack through D2. A Pi power stub (30 × 20 × 10, 12 V in from the logic board, 5 V out to the Pi's GPIO header; PLAN.md 6a, main e7d0b62)
sits on the compute shelf beside the Pi at X 22 to 52, Z 20 to 40, clear of the IMU. The FlexVolt options (D1 to D3) stay in `generator/variants.py` as the record of what was rejected.

Volumes: upper bay 156.8 × 100 × 101.4 between the RS03 rears; lower box 110 × 100 × 178.6 between the bottom rails and the shared rails; waist bay 150 × 140 × 40 below the frame.

| Component | Envelope | Where | Fits? |
| --- | --- | --- | --- |
| Pi 5 + CAN HAT | 85 × 56 × 21 | upper bay, on the shelf, ports rear (D1 door) | yes, as built |
| Jetson Orin Nano dev kit | 103 × 90.5 × 35, +10 for its fan | upper bay, second shelf at Y 410, ports rear | **yes**, 30 mm left under the top rails. Needs a shelf bracket on the shelf rails or the posts |
| Third compute board / USB hub | up to 90 × 60 × 20 | none yet | **no**: the pack and boards fill the lower box; pelvis or over the Pi |
| Shoulder pitch RS03 × 2 | Ø99 × 57 | through the upper box sides | yes (0.5 mm per side to the 100 inner depth, as built) |
| Waist yaw RS03 | Ø99 × 57 | 40 mm waist bay | **no**, 23 mm short including the 6 mm adapter plate (motor Y 92 to 149). Hip stage: pocket the adapter, add the +70 waist section, or redesign the pelvis around Joey's fixed cell |
| Power + logic boards (BMS, fuse, contactor, DC-DC, CAN) | 100 × 150 × 24 on one carrier | in front of the pack, Z 37.5 to 61.5 | yes, 8.5 mm behind the frame front face; **but** the corner bracket heads on the lower post inner faces (X ±46.5 to 51, Y 175 to 203 and 325 to 353) reach 3.5 mm into the board corners: board thread keep-out (gate 2) |
| Battery, 13S2P 21700 pack | 95 × 162 × 79 | lower box, \|X\| ≤ 47.5, Y 183 to 345 | **yes** (locked 06:13Z); retention is **open** (gate 2: the D2 door is not a restraint) |
| M18 HD12 aux bay (handoff draft) | 168 × 98 × 112 | upper back cantilever | **no**: the shell back has 10 to 17 mm to the frame and the spine pads sit there. Recommend dropping it or moving it to the pelvis |
| Fans | 40 × 40 × 10 | 2 exhaust, provisionally in the D1 door plane (Y 430 to 470); 2 intakes in the lower front panel (Y 264 to 304, 193 to 233) | provisional. Gate 2: the lower box has **no exhaust** (the shared rails and the shelf close it off from the upper bay); proposed exit via the V1 side grilles, shell thread |
| Sensors | IMU fitted; cameras/mics in the head later | shelf; top rails | yes |
| Harness | two 12–14 AWG + CAN trunks | side bands of the lower box, shoulder runs at Y 447 to 457 stopping at \|X\| 52 and crossing behind the Jetson | partly: no lower-to-upper passage yet (shelf notch proposed) |

With the 13S2P pack (95 × 162 × 79) the lower box holds the pack and the boards without widening; the open items from gate 2
(pack retention, the lower-to-upper cable passage, lower-box exhaust, bracket-head keep-outs) are listed in `review/GATE2_REVIEW.md`.

## 7. What changes from the built frame

0. (Power) No frame geometry change. The 13S2P pack and the power and bus board live in the lower box on two printed tray rails and a printed board carrier; how the rails attach and retain the pack is open (gate 2).

1. Cut the four 340 mm posts into 198.6 (lower) + 121.4 (upper).
2. New rails: 2 × 226 shared step rails, 2 × 186 top rails, 2 × 100 shelf rails.
3. Move the top side rails and the shelf side rails out 38 mm per side (they become the upper box and step side rails).
4. Add four 3 mm 6061-T6 gusset plates (front and rear of each step corner), 9 × M5 each.
5. Add 16 corner brackets (32 total: 28 × 2028 + 4 × 2020 at the top inner-X corners).
6. Move the two pitch brackets and RS03s out 38 mm with the upper box; output faces go from X ±97 to ±135.
7. Add 20 printed shell hardpoint brackets on T-nuts (shell thread) and the H6 position change.
8. Add a second compute shelf for the Jetson and reserve the PDB envelope behind the D2 door.
9. Compute shelf, Pi, bottom rails, bottom side rails and the waist adapter do not move.

## 8. Reconciliation record

| Point | Arm thread | Shell / manufacturing threads | This spec |
| --- | --- | --- | --- |
| Frame width at the shoulders | +38 per side, two-box | fits the locked shell with ≥ 15 mm (deeper back approved by Joey) | 226 upper, 150 lower |
| Gussets | 3 mm 6061-T6, 9 holes | not used as mounts | as arm thread, R15 at the step corner (agreed 04:29Z) |
| Rear crush | needs ~30 side / 40 back | TPU spine pads on H2/H3/H4 | pads as interim spec (04:29Z) |
| H6 | | under-face of the bottom side rails | **moved to the outer side faces** (agreed 04:29Z) |
| H3 rear | below Y 374 | agreed | kept |
| Pitch brackets | same parts, moved | | kept |
| Compute shelf / Pi / waist adapter | unchanged from the original torso CAD | | kept; footprint to verify on Joey-PC |

## 9. Open items (Joey)

0. **Power**: 13S2P custom pack locked (Joey 06:13Z). Next: the power and bus board design (KiCad), the pack build, and D2 sized to pass the pack. The third compute board has no home (pack and board fill the lower box): pelvis, or a second layer over the Pi.

1. ~~Photo of the built frame bottom~~ Joey 2026-11-06 10:36Z: the built frame is still just the bare 2020 box, no waist actuator and no
   mount yet, so nothing on it constrains the hip interface; the bottom rails' under-face is free. A photo would still confirm the rail
   count (12 or 16) and bracket positions. Shelf footprint is from the CAD.
   Photos 10:40Z in `reference/` (user-reported, read by eye, not measured): the bottom rectangle is four 2020 rails joined at the
   corners by **printed inside-corner gussets** lying in the rail plane, with printed L brackets at the rear post feet, not the
   aluminium 2028 cast brackets the generator counts. Printed foot tabs on the bottom rails (one mid-front, two splayed at the front
   corners) screw the frame to the bench stool. The bottom rails' under-face carries nothing. Consequences: (a) the cut list and
   bracket count describe the new frame, not the built one; the built printed corner pieces are reused or replaced by the 2028 set
   at the rebuild, Joey's call; (b) the foot tabs are bench fixtures and come off for the hip interface; (c) the rail count still
   cannot be read from these two photos (posts at the front corners look doubled in the side view), so the nut map keeps it open.
   Three more photos 10:42Z (read by eye): the posts are single 2020 (the doubled look is the post's slot lips); the rear post-to-rail
   joints use metal angle brackets (silver edge visible), the front ones printed gussets; the bottom ring looked like two stacked
   rails in the photos, but **Joey confirmed 10:50Z: the whole frame is single rail** (what looked stacked is the rail's face slots and
   the printed foot tabs). The built bottom ring matches the generator's single Y 155-175 ring; no geometry change.
1a. **Which connector flat each pitch RS03 has** (A or B above), for the harness exit.
2. ~~Main battery location~~ 13S2P in the lower box, locked 06:13Z. Gate 2 decisions for Joey: pack cradle (A), shelf corner notches for the cables (B), end-tapped step-level joints (C), post cuts before or after the hip decision (O); `review/GATE2_REVIEW.md`.
3. **M18 aux bay**: drop, or move to the pelvis.
4. Waist RS03 in the 40 mm bay and the +70 mm waist section: hip-stage decision, does not change this frame.
5. RS02/RS03 output bearing moment ratings from RobStride (arm thread's ask).

## 10. Before cutting

**Gate review 2, 2026-11-06 (rev A + 13S2P pack + boards; Wonder, Sol, Astra, Opus high, Argon High, Fable high): NO-GO stands.**
Reconciled in `review/GATE2_REVIEW.md` with per-reviewer times. Fixed: fastener seating and the missing 28 T-nuts, M5 × 18 / × 10
lengths, spine lip |X| ≤ 52, board standoffs and carrier, harness paths out of the aluminium and the Jetson, stale text and mass.
Open before any order or build: pack retention, the lower-to-upper cable passage, the step-level joints, lower-box exhaust, end
anchors, the nut map, and Joey's call on cutting before the hip decision.

Gate 1 (below) for the record:

**Gate review 2026-11-06 (Sol, Astra, Grok, Muse): NO-GO on the post cuts and the gusset plates.** Reconciled in
`review/GATE_REVIEW.md`. Fixed in this revision: the four lower-post step brackets were generated 38 mm off their post; T-nuts
were modelled 20 mm long; gusset mass; step side rail brackets ran into the shelf rails; hardpoint screws now M5 × 20; the Jetson
cooler and the fan positions; the DXF with the R15 corner now exists here. Still open before any cut: the built-rail inventory photo,
a slot-by-slot nut map and build order, the
step joint's capacity along the slots (friction only; reviewers want an end anchor), the back-fall crush path through the pads,
and the pack and bus board designs.

Gate review packet (reviewer CLIs, per the project rule): this README, `generator/parts.json`, the sheets, the cut list, the
torso-stepped FEA. Then the SolidWorks master on Joey-PC gets the shelf rails, second shelf, PDB envelope and hardpoint brackets,
and a native exploded view configuration is saved from it.

## Reproduce

In `generator/`: `npm install three@0.170.0`, `python3 frame.py`, `python3 gusset_dxf.py`, `python3 draw2d.py`, `python3 -m http.server 8766`, then `node shoot.js '[...]'`
(view list in `generator/views.json`), `python3 compose.py`, `python3 build_viewer.py`.
