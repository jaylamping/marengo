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

Total extrusion 3124 mm, of which 1024 mm is new stock. 32 corner brackets.

**D3 cut plan (2026-11-06 05:45Z).** The lower box is 20 mm taller (bottom Y 135), so a lower post is 218.62 and 218.62 + 121.38 = 340 leaves
nothing for a kerf. The four lower posts are therefore NEW (4 × 218.62), and the four upper posts (121.38) come out of TWO of the built
340 mm posts, two cuts each, 94 mm scrap per post. Two built 340s stay spare. New stock: 4 × 218.62 + 2 × 226 + 2 × 186 + 2 × 100 =
1898 mm; one more 4 × 500 mm pack (`B08Y8KCJW4`) nests it as 218.62 + 226, 218.62 + 226, 218.62 + 186, 218.62 + 186 (444.6 and 404.6
per stick, kerf included) with the two 100 mm shelf rails cut from the two spare 110 mm top rails. Spare after the rebuild: two 340 mm
posts, 2 × ~95 mm stick ends and 2 × 94 mm post offcuts.

**Joint count:** 32 corner brackets with 2 × M5 T-nuts each (bottom corners 8, lower post tops 4, upper post bottoms 4,
top corners 8, shelf rail ends 4, step side rail ends 4). 24 are 2028; 8 are 2020 brackets (20 mm legs): the 4 top inner-X corners,
because a 2028 there overlaps the pitch bracket wing at Y 447 to 450 (arm thread interference check), and the 4 step side rail ends,
because only 18 mm separates the step side rails from the shelf rails and a 2028 leg ran 10 mm into the shelf rail (gate review).
The built box uses 16, so 8 more 2028s. Optional +8 at the step if the post Z faces also get a bracket; the gate reviewers
noted that eight more friction joints do not by themselves carry the side-fall shear, which runs along the gusset slots.

**Fasteners (all on `FRAME_EXPLODED_DETAIL.png`):**

| Fastener | Qty | For |
| --- | --- | --- |
| M5 × 10 button head + M5 T-nut | 64 | corner brackets |
| M5 × 10 button head + M5 T-nut | 36 | gussets, 9 per plate |
| M5 × 16 button head + M5 T-nut | 20 | shell hardpoint brackets (through 12 mm of PETG) |
| M5 × 12 button head + M5 T-nut | 8 | pitch brackets to the upper posts (**assumption**: the built bracket's pattern decides) |
| RS03 rear mounting screws | 2 sets | vendor pattern, as built |
| M4 × 10 button head | 16 | shell collars on the RS03 outboard holes |
| M2.5 standoffs and screws | 8 + 4 | Pi 5 stack, Jetson carrier |
| M3 heat-set inserts + M3 × 8 | ~40 | shell panels to the hardpoint brackets (shell thread) |

Total: **128 M5 screws and 128 M5 T-nuts** on the frame.

## 2a. Shopping list

What the BOM (`hardware/bom/master-bom.csv`) says Joey bought against what rev A needs. **Confirm the stock on hand before ordering.**

| Item | Bought (BOM) | Needed for rev A | Buy |
| --- | --- | --- | --- |
| 2020 extrusion, 500 mm sticks | 2 × 4-pack = 4000 mm; 2400 mm cut for the built frame | 1024 mm new (2 × 226, 2 × 186, 2 × 100) | Depends on the inventory (the as-built list has 2200 mm and no shelf rails; this spec assumed 2400 mm). Buy the pack either way; the two spare 110 rails can become the two 100 shelf rails, which drops the new stock to 824 mm: **1 more 4-pack (`B08Y8KCJW4`)** |
| 2028 corner brackets | kit of 30, 16 used | 24 | 14 spare on hand: **buy none**, or one kit if any are bent |
| 2020 corner brackets, 20 mm legs | none | 8 | **1 pack of 10** (any 2020-series inside corner bracket, 20 × 20) |
| M5 T-nuts, 2020, 10 mm long | ~32 with the kit | 128 (+ spares) | **100-pack slide-in** for the corner brackets, plus **a 50-pack of drop-in** for the gussets, H3 and the pitch brackets, which go in after the posts are up (gate review: slide-ins cannot reach those slots once the posts clamp the rail ends) |
| M5 × 10 button head, stainless | some | 100 | **100-pack** (exact; add a 25-pack for spares) |
| M5 × 12 and M5 × 20 button head | none | 8 and 20 | **25-pack each**. M5 × 16 through the 12 mm PETG hardpoint brackets left about 2 mm of thread (gate review); the 12s for the pitch brackets are an assumption until the built bracket is measured |
| 3 mm 6061-T6 sheet | none | 4 plates 58 × 140 | **one 150 × 300 mm sheet**, or 4 plates laser-cut from `gusset_plate_left.dxf` in this folder (R15, generated by `generator/gusset_dxf.py`; mirror two). The older `../torso-stepped/gusset_plate_left.dxf` is R8 everywhere and is superseded |
| M4 × 10 button head | arm stock | 16 | from the arm fastener stock |
| M3 heat-set inserts (M3 × 5.7) + M3 × 8 | none | ~40 each | **50-pack each** (shell thread) |
| PETG, TPU 95A | printer stock | ~0.5 kg brackets and adapters, ~0.35 kg pads | top up as needed |
| 40 × 40 × 10 fans, 5 V or 12 V | none | 3 | **3** once the PDB voltage rails are fixed |
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
| 11 | Corner bracket 2028 + 2 M5 T-nuts | 24 | aluminium | 432 | 16 reused, 8 new |
| 34 | Corner bracket 2020 (20 mm legs), top inner-X corners and step side rail ends | 8 | aluminium | 112 | **new** |
| 12 | RS03 shoulder pitch | 2 | RobStride | 1760 | existing; Ø80 rear boss X 78.4 to 92 with two connector flats, Ø99 body to 132.5, output face 135 |
| 13 | Shoulder pitch bracket | 2 | PETG | 240 | existing part, moved out 38; envelope X 78.4 to 113, Y 353.6 to 495, Z ±50, wing to ±69 at Y 400 to 450 (recorded CAD check) |
| 14 | Compute shelf plate | 1 | existing | 60 | existing; X ±55 between the shelf rails, Y 354.6 to 372.6 (recorded CAD check) |
| 15 | Pi 5 + 2-CH CAN HAT, BNO085 | 1 | | 115 | existing; Y 376 to 397, X −37.5 to 19.7, ports to Z −71.8 (recorded CAD check) |
| 16 | PDB + E-stop / main switch envelope 100 × 90 × 28 | 1 | placeholder | 150 | proposed: lower box rear, behind the D2 door |
| 17 | RS03 waist yaw | 1 | RobStride | 880 | existing; 57 tall in a 40 bay (**unresolved**, hip stage) |
| 18 | Waist yaw adapter plate | 1 | PETG | 120 | existing; bolt pattern **unresolved** |
| 19–24 | Shell hardpoint brackets H1–H6, PETG with M3 inserts on M5 T-nuts | 20 | PETG | 160 | shell thread; H6 moved (section 5) |
| 25 | Shell collar halves on the RS03 outboard M4 holes | 2 pairs | PETG | 30 | shell part |
| 26 | TPU spine pads (2 upper, 2 lower, 2 bridges), 30 mm proud of the shell back, on H2/H3/H4 standoffs | 6 | TPU 95A | 333 | shell thread, main 074cab4 |
| 27 | Harness trunks: Pi/PDB to each shoulder RS03, two front-corner drops to the waist, step bus | 5 | 12–14 AWG + CAN pair | 220 | routing proposal |
| 28 | Pelvis ring | 1 | draft | | hip stage, shown only in the viewer |
| 29 | Jetson Orin Nano dev kit envelope 103 × 90.5 × 45 including its fan | 1 | proposed | 250 | second shelf, Y 410 to 455, ports to the rear; 20 mm to the top rails |
| 30 | Fans 40 × 40 × 10: 2 exhaust, 1 intake lower front | 3 | proposed | 60 | exhaust position PROVISIONAL: in the D1 door plane behind the frame face (Y 430 to 470, Z −70 to −80); inside the bay they hit the top rail. The power-board rework will redo the cooling layout |
| 31 | Third compute / USB hub board envelope 90 × 60 × 20 | 1 | proposed | 60 | lower box front |
| 32 | OPTION: in-torso battery, 2 × 6S 5000 mAh standing in the lower box | 2 | option | 1500 | baseline keeps the pack in the pelvis |
| 33 | Sensors: IMU on the shelf; head cameras/mics future on the top rails | | | | |

Frame structure only (1 to 11): about **2.1 kg** including brackets and T-nuts. Everything shown: about 7 kg without the battery option.

### Interference check against the real CAD (arm thread, 2026-11-06 04:46Z)

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
| H2 | Top rear rail, rear face, X ±60, Y 485 | 2 | U2 top, D3 catch, spine pads | free |
| H3 | Shared rail front and rear faces, X ±35, Y 364 | 4 | U1/U2 bottom, U3/U4 top, D1 frame, spine pads | free; **rear brackets stay below Y 374** (Pi ports) |
| H4 | Bottom rails front and rear faces, X ±40, Y 145 (D3) | 4 | U3/U4 bottom, D2 frame, spine pads | free |
| H5 | Lower post outer side faces, X ±75, Y 255, Z ±60 | 4 | U3/U4 sides, V1 grilles | free (gusset starts at Y 285) |
| H6 | **Moved:** bottom side rail outer faces, X ±75, Y 145 (D3), Z ±35 | 4 | abdomen band carrier | agreed; the under-face stays free for the hip interface |
| C | Pitch RS03 outboard M4 holes | 2 | collars | free |

Gussets are not mounts. Doors: D1 at |X| < 62, Y 360 to 492 (rear, the shell thread's current extent); D2 at |X| < 58, Y 167 to 295 (rear). The spine pads (30 mm
TPU, Z −94 to −125) flank the doors: upper |X| 62 to 135, Y 370 to 480; lower |X| 58 to 110, Y 170 to 295; bridge |X| < 60,
Y 300 to 355. They give the 30 to 40 mm of rear crush the tip-over FEA needs (`../torso-stepped/README.md`).

## 6. Internal packaging check (Joey's list, 2026-11-06)

**D3 layout (Joey 05:45Z): two FlexVolt-class 60 V packs end-on through the back.** Pack envelope 160 × 102 × 95, 1.5 kg each (DeWalt
DCB6112 class; Joey to confirm the pack). Both stack in the lower box at |X| ≤ 51 (4 mm to the posts), Y 158 to 253 and 257 to 352,
Z −114 to +46: the terminal end faces forward onto a dock on the front rail plane (Z 46 to 58, a printed plate with a DeWalt-pattern
rail adapter), the rear face stands 44 mm proud of the frame rear face (Z −70), which is 7 to 17 mm past the present shell back line.
A pack slides straight out the back; nothing in the frame is behind it. The power board (100 × 150 × 24, not designed) stands in front
of the docks at Z 58 to 82, X ±50, Y 180 to 330, 12 mm proud of the frame front face, with the intake fan on it at Y 235 to 275 in the
front panel grille. Electrical: a 15S pack is 63 V full, over the RS03 60 V limit, so the two packs run in 20 V mode in series (10S,
36 V nominal, 42 V full, 432 Wh); the power board does low-voltage cutoff, per-pack fuse, precharge, ideal-diode OR-ing for hot swap
and the 48/12/5 V rails. The fixed pelvis cell Joey asked for is a hip-stage item. Total pack mass 3.0 kg plus docks.

Volumes: upper bay 156.8 × 100 × 101.4 between the RS03 rears; lower box 110 × 100 × 198.6 between the bottom rails and the shared rails (D3, bottom Y 135); waist bay 150 × 140 × 40 below the frame.

| Component | Envelope | Where | Fits? |
| --- | --- | --- | --- |
| Pi 5 + CAN HAT | 85 × 56 × 21 | upper bay, on the shelf, ports rear (D1 door) | yes, as built |
| Jetson Orin Nano dev kit | 103 × 90.5 × 35, +10 for its fan | upper bay, second shelf at Y 410, ports rear | **yes**, 30 mm left under the top rails. Needs a shelf bracket on the shelf rails or the posts |
| Third compute board / USB hub | up to 90 × 60 × 20 | lower box front, standing | yes |
| Shoulder pitch RS03 × 2 | Ø99 × 57 | through the upper box sides | yes (0.5 mm per side to the 100 inner depth, as built) |
| Waist yaw RS03 | Ø99 × 57 | 40 mm waist bay | **no**: with the D3 bottom at Y 135 the waist bay is 20 mm and the motor (Y 72 to 129) sits 43 mm into the draft pelvis. Hip stage: the pelvis is being redesigned anyway for Joey's fixed cell |
| PDB + converters + E-stop + main switch | 100 × 90 × 28 assumed | lower box rear, behind D2 | yes; the board is not designed yet |
| Battery, main 48 V pack | 185 × 165 × 90 draft | pelvis | **not in the torso frame**. The lower box can take two 6S 5000 mAh packs standing (155 × 50 × 45 each, ~1.5 kg) if Joey wants torso batteries, at the cost of the PDB depth and a higher centre of mass |
| M18 HD12 aux bay (handoff draft) | 168 × 98 × 112 | upper back cantilever | **no**: the shell back has 10 to 17 mm to the frame and the spine pads sit there. Recommend dropping it or moving it to the pelvis |
| Fans | 40 × 40 × 10 | 2 exhaust, provisionally in the D1 door plane (Y 430 to 470, behind the frame face; a 40 mm fan does not fit the 20 mm left above the Jetson cooler), 1 intake in the lower front panel at Y 250 to 290 | provisional; 45 W chest heat estimate in the shell plan, plus the Jetson's 7 to 15 W |
| Sensors | IMU fitted; cameras/mics in the head later | shelf; top rails | yes |
| Harness | two 12–14 AWG + CAN trunks | front inner corners of the lower box, step bus to the PDB, shoulder runs at Y 460 | yes, T-slot clips; keep off the H1–H6 slots |

The frame is adequate for this list **with the main battery in the pelvis**. If Joey wants the main pack in the torso, the lower
box is too small and this spec needs a wider lower box, which also changes the slim waist of the shell. That is the one decision
that would reopen the frame.

## 7. What changes from the built frame

0. (D3) Frame bottom moves from Y 155 to Y 135: the torso is 360 tall, the shoulder axis 289.9 above the frame bottom. Four new 218.6 lower posts; upper posts cut from two built posts. Two FlexVolt-class packs in the lower box, docks on the front rail plane, power board on the front face. H4 and H6 move down 20 (Y 145).

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

0. **Battery**: D3 chosen (`FRAME_OPTIONS_D.png`, Joey 05:45Z). Confirm the pack model (height especially) so the dock and the bay are sized to it. The third compute board has no home in D3 (packs fill the lower box): pelvis, or a second layer over the Pi.

1. **Photo of the built frame bottom**: how the waist adapter bolts on (the hip interface depends on it). Shelf footprint is now from the CAD.
1a. **Which connector flat each pitch RS03 has** (A or B above), for the harness exit.
2. **Main battery location**: pelvis (baseline) or torso (reopens the lower box width).
3. **M18 aux bay**: drop, or move to the pelvis.
4. Waist RS03 in the 40 mm bay and the +70 mm waist section: hip-stage decision, does not change this frame.
5. RS02/RS03 output bearing moment ratings from RobStride (arm thread's ask).

## 10. Before cutting

**Gate review 2026-11-06 (Sol, Astra, Grok, Muse): NO-GO on the post cuts and the gusset plates.** Reconciled in
`review/GATE_REVIEW.md`. Fixed in this revision: the four lower-post step brackets were generated 38 mm off their post; T-nuts
were modelled 20 mm long; gusset mass; step side rail brackets ran into the shelf rails; hardpoint screws now M5 × 20; the Jetson
cooler and the fan positions; the DXF with the R15 corner now exists here. Still open before any cut: the built-rail inventory photo,
the real pitch-bracket hole heights (they may land on the top corner bracket nuts), a slot-by-slot nut map and build order, the
step joint's capacity along the slots (friction only; reviewers want an end anchor), the back-fall crush path through the pads,
and the battery decision above.

Gate review packet (reviewer CLIs, per the project rule): this README, `generator/parts.json`, the sheets, the cut list, the
torso-stepped FEA. Then the SolidWorks master on Joey-PC gets the shelf rails, second shelf, PDB envelope and hardpoint brackets,
and a native exploded view configuration is saved from it.

## Reproduce

In `generator/`: `npm install three@0.170.0`, `python3 frame.py`, `python3 gusset_dxf.py`, `python3 draw2d.py`, `python3 -m http.server 8766`, then `node shoot.js '[...]'`
(view list in `generator/views.json`), `python3 compose.py`, `python3 build_viewer.py`.
