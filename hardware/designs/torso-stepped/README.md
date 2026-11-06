# Stepped torso frame

> Release spec and full exploded view: [`../torso-frame/`](../torso-frame/README.md) (thread "Finalize the internal torso frame"). This folder stays the FEA and study record.

2026-11-06. Two-box 2020 frame: a wide upper box at the shoulders on the existing lower cage. This is the master, locked with the shell baseline. The skeleton, layout, URDF (shoulder at ±208.49) and envelope all use it. The pre-step backup is on Joey-PC.

Coordinates: torso STEP frame, mm. X lateral, Y up, Z forward. Y is measured from the pelvis bottom.

## Step height

| Item | Y | From shoulder axis |
| --- | --- | --- |
| Shoulder axis | 424.86 | 0 |
| Top of upper box | 495 | +70.14 |
| **Bottom of upper box, shared rail underside** | **353.62** | **-71.24** |
| Shared rail top | 373.62 | -51.24 |
| Gusset plate bottom | 285 | -139.86 |
| Bottom of lower frame | 155 | -269.86 |

| Width | Half-width X |
| --- | --- |
| Upper box outer | 113 |
| Lower box outer | 75 |
| Step | 38 |
| Pitch RS03 output face | 135 |
| Arm joint line | 208.49 |

Depth is ±70 over 2020, the same in both tiers.

## Gusset plate

`gusset_plate_left.step` and `gusset_plate_left.dxf`. Four plates, two per step corner, on the front and rear faces. The files are the left side at the front. Mirror X for the right.

| Item | Value |
| --- | --- |
| Outline points, X Y | 55 285, 75 285, 113 335, 113 425, 93 425, 93 373.62, 55 373.62 |
| Corner radius | 8 |
| Holes | 9 × Ø5.5 for M5 T-nuts |
| Lower post holes | X 65, Y 297, 317, 337 |
| Shared rail holes | Y 363.62, X 62, 84, 104 |
| Upper post holes | X 103, Y 385, 400, 415 |
| Sits on | Z ±70 frame faces |
| Outer face, 10 mm plate | Z ±80 |

The outline and thickness may change after the fall-case study.

## FEA so far

The frame model is one merged body with 20 × 20 × 2 mm tube for each 2020, so joints are perfectly bonded. Real corner brackets and T-nuts are softer and can slip. Treat frame deflections as lower bounds, and use percentiles rather than the peaks, which sit at sharp corners. Frame material is 6063-T5, yield 145 MPa, assumed.

| Case | Load | Applied at |
| --- | --- | --- |
| Peak | Each arm 60 N·m pitch plus 330 N down (279 N from the torque, 51 N from 2 × gravity) | Arm joint line, X ±208.49. Base fixed |
| Tip-over, side | 5.3 kN lower body, 1.2 kN far arm | Near box side fixed |
| Tip-over, forward | 5.3 kN lower body | Both box sides fixed |

**Tip-over basis:** ground-level fall from standing, 40 kg (assumed), about 0.75 m centre-of-mass drop, 294 J. About 60% arrives at first contact, stopped over 40 mm of shell and link crush, with a peak of 1.5 × average. That gives about 6.6 kN, or 17 g. It scales inversely with crush distance, so 80 mm halves it.

### Frame

| Case | Gusset | Displacement, mm | Peak, MPa | 99th percentile, MPa |
| --- | --- | --- | --- | --- |
| Peak | 3 mm aluminium | 0.08 | 22.7 | 7 |
| Peak | None | 0.08 | 25.6 | 10 |
| Tip-over, side | 3 mm aluminium | 1.8 | 297 | 100 |
| Tip-over, side | None | 5.3 | 434 | 196 |
| Tip-over, forward | 3 mm aluminium | 1.8 | 277 | 117 |
| Tip-over, forward | None | 3.1 | 429 | 168 |

Normal use does not need the gusset. The tip-over does: without it, the 99th percentile is over yield.

### Plate alone

One plate carrying the whole share, with no help from the shared rail. In-plane stress scales with 1/thickness, so the aluminium rows are scaled from the 10 mm run, not solved. Printed flat, layers parallel to the plate.

| Plate | Case | Peak, MPa | Safety factor | Layer safety factor | Displacement, mm |
| --- | --- | --- | --- | --- | --- |
| 10 mm PETG | Peak | 17.0 | 2.6 | 3.2 | 0.55 |
| 10 mm PETG | Tip-over | 36.9 | 1.2 | 1.5 | 0.43 |
| 3 mm 6061-T6 | Tip-over, scaled | 123 | 2.2 | | 0.04 |
| 4 mm 6061-T6 | Tip-over, scaled | 92 | 3.0 | | 0.03 |

Bolts in the tip-over: 4.3 kN per plate over 5 bolts is about 860 N each. Bearing is 17 MPa on 10 mm PETG and 57 MPa on 3 mm aluminium. Friction on an M5 T-nut is only about 1 kN, assumed, and clamping through PETG relaxes, so printed plates are likely to slip onto the bolts.

**Printed plates survive but do not protect the frame.** 10 mm PETG has about 9% of the in-plane stiffness of 3 mm aluminium, so the frame still takes nearly the no-gusset load. Estimated side displacement is about 5 mm against 1.8 mm. **Recommendation: 3 mm 6061-T6 in the same outline**, with a larger radius at the X 113, Y 335 corner, where the 297 MPa peak sits.

## Options for the mid-tier

Joey's rule: aluminium 2020 at the shoulder box and the hip frame, printed in between if it costs little strength. Both options cover Y 175 to 353.62, with rigid ends and the same loads. PETG is printed upright.

| | A: four 2020 posts | B: printed box |
| --- | --- | --- |
| Section | 2020 at X ±65, Z ±60 | 150 × 140, 6 mm wall, 80 × 120 windows front and rear |
| Mass | 357 g | 616 g |
| Peak use, displacement | 0.02 mm | 0.22 mm |
| Peak use, safety factor | 23 | 15 (layer 9.4) |
| Side tip-over, displacement | 1.3 mm | 5.8 mm |
| Side tip-over, peak | 197 MPa, yield 145 | 73 MPa, yield 45 |
| Side tip-over, layer safety factor | | **0.34 at window corners** |
| Forward tip-over, displacement | 1.3 mm | 1.9 mm |
| Forward tip-over, peak | 198 MPa | 24 MPa |
| Forward tip-over, layer safety factor | | 0.75 at the joint edge, 1.4 away from it |

**Stay with A.** B is 260 g heavier, 12 times softer in normal use, and cracks across the layers at the window corners in a side fall. Total shoulder deflection would go from about 0.08 to about 0.29 mm. A smaller window would help B but costs access. Printing stays better suited to brackets, covers and the shell.

A's 99th percentile sits at yield in the fall (145 MPa) with bonded ends. Real corner brackets will slip before that, so check them after a fall.

## Cut list

Reuses the current frame wherever the length allows.

| Member | Qty | Length, mm | Source |
| --- | --- | --- | --- |
| Lower posts, Y 155 to 353.62 | 4 | 198.62 | Cut from the 340 mm posts |
| Upper posts, Y 373.62 to 495 | 4 | 121.38 | Offcuts of the same posts |
| Shared rails, front and rear, Y 353.62 to 373.62 | 2 | **226** | **New** |
| Top rails, front and rear | 2 | **186** | **New** |
| Bottom rails, front and rear | 2 | 110 | Existing |
| Top side rails, moved to X ±103 | 2 | 100 | Existing |
| Step side rails, X ±103, Y 353.62 to 373.62 | 2 | 100 | Existing shelf side rails |
| Shelf rails, X ±65, Y 353.62 to 373.62 | 2 | **100** | **New** |
| Bottom side rails | 2 | 100 | Existing |
| Gusset plates, 3 mm 6061-T6 | 4 | | **New**, 40 g total |

Spare: two 110 mm rails and four 20 mm offcuts.

New extrusion is 1024 mm. Total goes from 2460 to 3124 mm, about 332 g more, assuming 0.5 g/mm. Add 40 g for gussets plus 36 M5 T-nuts and bolts.

## What lands where

| Item | Result |
| --- | --- |
| Compute shelf and Pi | No change. New rails at X ±65 keep the 110 mm span |
| Pitch brackets | Same parts, moved out 38 mm to the X ±103 rails |
| Waist adapter | No change. The bottom rectangle stays ±75 |
| Side room below the step | Frame steps in 38 mm per side below Y 353.62. The arm clears the shell, not the frame, so this frees the shell to taper at the waist. It adds no roll range by itself |

## Tip-over against crush distance

The shell only has 8 to 17 mm to the frame at the back and sides, so 40 mm of crush was optimistic. Linear scaling from the 3 mm aluminium runs. Limit used: 99th percentile at 145 MPa yield.

Back case: frame held on the rear rails of the upper box, 5.3 kN of lower body, 3.2 mm, 99th percentile 146 MPa. See `STEPPED_FALL_BACK_3MM.png`. Holding one face is harsher than a real landing on the whole shell.

| Stop | Peak factor | Load | Side, 99th pct | Back, 99th pct | Holds? |
| --- | --- | --- | --- | --- | --- |
| 10 mm shell gap | 1.5 | 26 kN | 400 MPa | 584 MPa | No |
| 10 mm gap + 10 mm TPU | 1.2 | 10.6 kN | 160 MPa | 234 MPa | No, local yield |
| 20 mm of rigid crush | 1.5 | 13 kN | 200 MPa | 292 MPa | No |
| 30 mm | 1.5 | 8.8 kN | 133 MPa | 194 MPa | Side only |
| 40 mm | 1.5 | 6.6 kN | 100 MPa | 146 MPa | Side yes, back at the limit |

The frame needs about 30 mm of stop at the side and 40 mm at the back. A side fall lands on the arm first, which stands more than 100 mm proud of the frame, so 30 mm is reachable there if the printed arm parts crush. The back has nothing proud of it, so it needs a pad. Bolted joints will slip or tear before these stresses matter, so treat 2020 and its T-nuts as parts to inspect after any fall.

## Shell hardpoints

From `body-shell/concept/generator/mounts.json`. Torso Y = robot Z - 810, torso X = robot Y, torso Z = robot X.

| Point | On | Status |
| --- | --- | --- |
| H1, H2 | Top rails, X ±60, Y 485, Z ±70 | Free. Gusset stops at X 93 and Y 425 |
| H3 | Shared rail, X ±35, Y 364, Z ±70 | Free on the rail. **At the rear, stay under Y 374:** the Pi reaches Z -71.8 from X -37.5 to 19.7, Y 376 to 397 |
| H4 | Bottom rails, X ±40, Y 165, Z ±70 | Free |
| H5 | Lower post side faces, Z ±60, Y 255, X ±75 | Free. Gusset starts at Y 285 |
| H6 | Bottom side rails, under face, Z ±45, X ±65, Y 155 | **Check on the robot:** the waist adapter is not in CAD and bolts somewhere on this rectangle |
| C | Pitch RS03 outer ring | Free. Button heads are the P2 item in `../left-arm/upper-arm/README.md` |

The PDB and harness clips are not in the CAD, so those cannot be checked yet. Keep them off these slots. Pi ports at the rear also need plug room inside the 8 to 17 mm shell gap.

## Pitch RS03 connectors

Each motor has a plug position on both rear flats, the same on both sides of the torso. This is how they are wired today, but both do not have to stay in use (Joey, 2026-11-06).

| Flat | Y | Z | Cable |
| --- | --- | --- | --- |
| Top rear | 447 to 456 | -31 to -23 | Out to the arm |
| Low front | 394 to 402 | 23 to 31 | Feed from the power supply |

Connector openings are about X ±83. Each flat has 11 to 12 mm of radial room to the pitch bracket, and cables leave inboard past X ±78.4. Keep both exits clear until the harness is designed, then release whichever goes unused:

| Keep-out | X | Y | Z |
| --- | --- | --- | --- |
| Top rear | ±52 to 78 | 445 to 460 | -45 to -15 |
| Low front | ±50 to 78 | 385 to 410 | 15 to 40 |
