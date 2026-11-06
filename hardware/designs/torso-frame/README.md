# Internal torso frame, rev A (release spec)

2026-11-06. Thread "Finalize the internal torso frame". One frame that the arm thread (`../torso-stepped/`, the FEA and study
record), the shell concept (`../body-shell/concept/`) and the shell manufacturing plan (`../body-shell/manufacturing/`) all agree
on. **Status: spec for Joey's inspection, not yet cut.** Gate review before any aluminium is cut (section 10).

Coordinates: torso STEP frame, mm. **X lateral (robot left +), Y up from the pelvis bottom, Z forward.** Robot frame: robot X =
torso Z, robot Y = torso X, robot Z = torso Y + 810.

| Sheet | Shows |
| --- | --- |
| `FRAME_EXPLODED.png` | Full exploded view, every part numbered to the table (section 3) |
| `FRAME_ASSEMBLED.png` | Assembled, front-left and back-right |
| `FRAME_DIMENSIONS.png` | Front, side, top with dimensions |
| `FRAME_SUBSYSTEMS.png` | Exploded by subsystem: structure, inside, shell interfaces |
| `FRAME_CHANGES.png` | Built frame vs rev A at the same scale |
| `FRAME_PACKAGING.png` | Internal packaging check (section 6) |
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

The four built 340 mm posts become 198.62 + 121.38 (kerf comes out of the 20 mm offcut). New stock: 2 × 226, 2 × 186, 2 × 100 =
1024 mm; one more 4 × 500 mm pack (`B08Y8KCJW4`) covers it. Spare after the rebuild: 2 × 110 rails and four 20 mm offcuts.

**Joint count:** 32 × 2028 corner brackets with 2 × M5 T-nuts each (bottom corners 8, lower post tops 4, upper post bottoms 4,
top corners 8, shelf rail ends 4, step side rail ends 4). The built box uses 16, so 16 more. Optional +8 at the step if the post
Z faces also get a bracket. The gussets add 36 × M5 × 10 + T-nuts.

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
| 10 | Gusset plate (`../torso-stepped/gusset_plate_left.dxf`, R15 at the X 113 / Y 335 corner) | 4 | 3 mm 6061-T6 | 40 | **new** |
| 11 | Corner bracket 2028 + 2 M5 T-nuts | 32 | aluminium | 576 | 16 reused, 16 new |
| 12 | RS03 shoulder pitch | 2 | RobStride | 1760 | existing |
| 13 | Shoulder pitch bracket | 2 | PETG | 240 | existing part, moved out 38 (envelope shown) |
| 14 | Compute shelf plate | 1 | existing | 60 | existing; footprint **unresolved** (section 9) |
| 15 | Pi 5 + 2-CH CAN HAT, BNO085 | 1 | | 115 | existing; Y 376 to 397, X −37.5 to 19.7, ports to Z −71.8 (recorded CAD check) |
| 16 | PDB + E-stop / main switch envelope 100 × 90 × 28 | 1 | placeholder | 150 | proposed: lower box rear, behind the D2 door |
| 17 | RS03 waist yaw | 1 | RobStride | 880 | existing; 57 tall in a 40 bay (**unresolved**, hip stage) |
| 18 | Waist yaw adapter plate | 1 | PETG | 120 | existing; bolt pattern **unresolved** |
| 19–24 | Shell hardpoint brackets H1–H6, PETG with M3 inserts on M5 T-nuts | 20 | PETG | 160 | shell thread; H6 moved (section 5) |
| 25 | Shell collar halves on the RS03 outboard M4 holes | 2 pairs | PETG | 30 | shell part |
| 26 | TPU spine pads (2 upper, 2 lower, 1 bridge), 30 mm, outside the shell on H2/H3/H4 standoffs | 5 | TPU 95A | 410 | shell thread interim spec |
| 27 | Harness trunks: Pi/PDB to each shoulder RS03, two front-corner drops to the waist, step bus | 5 | 12–14 AWG + CAN pair | 220 | routing proposal |
| 28 | Pelvis ring | 1 | draft | | hip stage, shown only in the viewer |
| 29 | Jetson Orin Nano dev kit envelope 103 × 90.5 × 35 (+10 fan) | 1 | proposed | 250 | second shelf, Y 410 to 445, ports to the rear |
| 30 | Fans 40 × 40 × 10: 2 exhaust upper rear, 1 intake lower front | 3 | proposed | 60 | |
| 31 | Third compute / USB hub board envelope 90 × 60 × 20 | 1 | proposed | 60 | lower box front |
| 32 | OPTION: in-torso battery, 2 × 6S 5000 mAh standing in the lower box | 2 | option | 1500 | baseline keeps the pack in the pelvis |
| 33 | Sensors: IMU on the shelf; head cameras/mics future on the top rails | | | | |

Frame structure only (1 to 11): about **2.1 kg** including brackets and T-nuts. Everything shown: about 7 kg without the battery option.

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
| H4 | Bottom rails front and rear faces, X ±40, Y 165 | 4 | U3/U4 bottom, D2 frame, spine pads | free |
| H5 | Lower post outer side faces, X ±75, Y 255, Z ±60 | 4 | U3/U4 sides, V1 grilles | free (gusset starts at Y 285) |
| H6 | **Moved:** bottom side rail outer faces, X ±75, Y 165, Z ±35 | 4 | abdomen band carrier | agreed; the under-face stays free for the hip interface |
| C | Pitch RS03 outboard M4 holes | 2 | collars | free |

Gussets are not mounts. Doors: D1 at |X| < 62, Y 360 to 480 (rear); D2 at |X| < 58, Y 167 to 295 (rear). The spine pads (30 mm
TPU, Z −94 to −125) flank the doors: upper |X| 62 to 135, Y 370 to 480; lower |X| 58 to 110, Y 170 to 295; bridge |X| < 60,
Y 300 to 355. They give the 30 to 40 mm of rear crush the tip-over FEA needs (`../torso-stepped/README.md`).

## 6. Internal packaging check (Joey's list, 2026-11-06)

Volumes: upper bay 156.8 × 100 × 101.4 between the RS03 rears; lower box 110 × 100 × 198.6; waist bay 150 × 140 × 40 below the frame.

| Component | Envelope | Where | Fits? |
| --- | --- | --- | --- |
| Pi 5 + CAN HAT | 85 × 56 × 21 | upper bay, on the shelf, ports rear (D1 door) | yes, as built |
| Jetson Orin Nano dev kit | 103 × 90.5 × 35, +10 for its fan | upper bay, second shelf at Y 410, ports rear | **yes**, 30 mm left under the top rails. Needs a shelf bracket on the shelf rails or the posts |
| Third compute board / USB hub | up to 90 × 60 × 20 | lower box front, standing | yes |
| Shoulder pitch RS03 × 2 | Ø99 × 57 | through the upper box sides | yes (0.5 mm per side to the 100 inner depth, as built) |
| Waist yaw RS03 | Ø99 × 57 | 40 mm waist bay | **no**, 17 mm short. Hip stage must pocket the adapter or add the +70 waist section |
| PDB + converters + E-stop + main switch | 100 × 90 × 28 assumed | lower box rear, behind D2 | yes; the board is not designed yet |
| Battery, main 48 V pack | 185 × 165 × 90 draft | pelvis | **not in the torso frame**. The lower box can take two 6S 5000 mAh packs standing (155 × 50 × 45 each, ~1.5 kg) if Joey wants torso batteries, at the cost of the PDB depth and a higher centre of mass |
| M18 HD12 aux bay (handoff draft) | 168 × 98 × 112 | upper back cantilever | **no**: the shell back has 10 to 17 mm to the frame and the spine pads sit there. Recommend dropping it or moving it to the pelvis |
| Fans | 40 × 40 × 10 | 2 exhaust at the upper rear under the top rail, 1 intake lower front | yes; 45 W chest heat estimate in the shell plan |
| Sensors | IMU fitted; cameras/mics in the head later | shelf; top rails | yes |
| Harness | two 12–14 AWG + CAN trunks | front inner corners of the lower box, step bus to the PDB, shoulder runs at Y 460 | yes, T-slot clips; keep off the H1–H6 slots |

The frame is adequate for this list **with the main battery in the pelvis**. If Joey wants the main pack in the torso, the lower
box is too small and this spec needs a wider lower box, which also changes the slim waist of the shell. That is the one decision
that would reopen the frame.

## 7. What changes from the built frame

1. Cut the four 340 mm posts into 198.6 (lower) + 121.4 (upper).
2. New rails: 2 × 226 shared step rails, 2 × 186 top rails, 2 × 100 shelf rails.
3. Move the top side rails and the shelf side rails out 38 mm per side (they become the upper box and step side rails).
4. Add four 3 mm 6061-T6 gusset plates (front and rear of each step corner), 9 × M5 each.
5. Add 16 corner brackets (32 total).
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

1. **Photo of the built frame bottom and compute shelf**: rail count, shelf footprint, how the waist adapter bolts on (H6 and the hip interface depend on it).
2. **Main battery location**: pelvis (baseline) or torso (reopens the lower box width).
3. **M18 aux bay**: drop, or move to the pelvis.
4. Waist RS03 in the 40 mm bay and the +70 mm waist section: hip-stage decision, does not change this frame.
5. RS02/RS03 output bearing moment ratings from RobStride (arm thread's ask).

## 10. Before cutting

Gate review packet (reviewer CLIs, per the project rule): this README, `generator/parts.json`, the sheets, the cut list, the
torso-stepped FEA. Then the SolidWorks master on Joey-PC gets the shelf rails, second shelf, PDB envelope and hardpoint brackets,
and a native exploded view configuration is saved from it.

## Reproduce

In `generator/`: `npm install three@0.170.0`, `python3 frame.py`, `python3 -m http.server 8766`, then `node shoot.js '[...]'`
(view list in the commit), `python3 compose.py`, `python3 build_viewer.py`.
