# Left forearm, draft 4: concept B with the 6808 support pair (option A)

2026-11-07. **Not released, not reviewed.** CAD is on Joey-PC in `cad/parts/left_arm/`. Coordinates are arm coordinates: elbow axis along X at Y 209.86, Z 5.99; forearm axis along Y at X 170.49, Z 5.99. Part files use a local frame with the same Y and the forearm axis through the origin.

Concept B ("muscle belly", 195 mm, `concepts/`) was chosen. Option A was chosen for the RS00: a pair of 6808-2RS bearings carries the hand bending moment around the RS00, so the RS00 output bearing carries only torque. The choice is drawn in `rs00_support_options.png`.

## Parts

| Part | File | Mass, solid | What it does |
| --- | --- | --- | --- |
| Fork B | `marengo_forearm_b_left` | 184 g | Belly shell from the elbow cheeks to the RS00. Cut at Y 90.3 so the RS00 goes in from below. Ø72 rim with 6 Ruthex M3 inserts on r 32.6. Cable channel in the back wall. |
| RS00 ring | `marengo_forearm_rs00_ring_left` | 19.7 g | Ø72 × 6 ring, Y 84.3–90.3. Bolted to the RS00 housing (6 × M3 on Ø50) on the bench. Clamped between the fork rim and the sleeve flange. |
| Sleeve | `marengo_forearm_sleeve_left` | 84.9 g | Static bearing barrel, Y 24.4–84.3. Bore Ø52.2 for both 6808s. A solid lip (r 24.5, Y 76–84.3) holds the upper bearing. 3 inserts at the bottom for the wrist cap. Flange bolts through the ring into the fork rim. |
| Tube | `marengo_forearm_tube_left` | 41 g | Turning inner tube, Ø40 journals, Ø34 bore for hand cables. Collar between the bearings. 3 dog slots at the top, 6 inserts at the bottom. |
| RS00 coupling | `marengo_forearm_rs00_coupling_left` | 7 g | Plate on the RS00 output (6 × M3 on Ø27). 3 dogs in the tube slots with 0.15 mm per side, so it passes torque only. |
| Wrist cap | `marengo_forearm_wrist_cap_left` | 6.5 g | Holds the lower 6808 from below. 3 × M3 into the sleeve inserts. Yaw stop lug. |
| Wrist flange | `marengo_forearm_wrist_flange_left` | 18.6 g | Turns with the tube. Spigot seats on the tube end. Hand pattern: 6 inserts on Ø50, Ø35 pilot. Yaw stop tab: ±170°. |

Bought parts are in `../BUY_ORDER.md` section 6. Whole forearm with the RS00: 779 g, CoM 102.3 mm below the elbow axis (100% infill CAD estimate; `testing/CALIBRATION_LOOP.md` section 14).

Section drawing with the cable route: `forearm_A_section.png`.

## Load path

- **Bending** (5 kg at the hand): tube → both 6808s → sleeve → sleeve flange and ring → 6 × M3 into the fork rim → fork → elbow.
- **Yaw torque:** RS00 output → coupling dogs → tube. RS00 housing → ring → fork screws and clamp friction → fork.
- **Axial:** when the hand hangs, load goes through the lower bearing to the wrist cap and its 3 screws. When the arm is overhead, load goes through the upper bearing to the sleeve lip.

## Assembly order and driver access (standard check, every new part)

Every new part gets this check from now on, run in assembly order:

1. **Insertion path:** ray-check or sweep the path the part goes in by.
2. **Screw-head entry:** confirm each screw head can reach its seat.
3. **Driver access:** confirm a 2.5 mm hex key reaches each socket. The script tries a straight key first, then a ball-end tilt of up to 25°.

Script: `fa_access.py` (in the left-arm task folder on Joey-PC). Results, 2026-11-08: every screw reachable, both insertion paths clear.

| Step | What | Path / driver |
| --- | --- | --- |
| 0 | Heat-set inserts: fork rim 6, sleeve bottom 3, tube end 6, wrist flange 6 | Open faces |
| 1 | Bench: RS00 ring onto the RS00 housing, 6 × M3 × 6, heads in counterbores | Straight key |
| 2 | Bench: coupling onto the RS00 output, 6 × M3 × 6 | Straight key |
| 3 | RS00 + ring + coupling up into the fork from below | 0 of 480 rays blocked |
| 4 | Sleeve up under the ring, 6 × M3 × 14 through the sleeve flange and ring into the fork rim inserts | **2.5 mm ball-end hex key**, 12–13° tilt outward: a straight key hits the sleeve body (r 32) |
| 5 | Upper 6808 onto the tube from the top. Tube up into the sleeve until the bearing seats on the lip, dogs into the coupling | 0.05 mm radial clearance in the bore (Ø52 race, Ø52.2 bore) |
| 6 | Lower 6808 onto the tube and into the sleeve from below. Lay the clock-spring ribbon first | 0.05 mm radial clearance |
| 7 | Wrist cap, 3 × M3 × 6 button head into the sleeve inserts | Straight key |
| 8 | Wrist flange onto the tube end, 6 × M3 × 10 button head into the tube inserts | Straight key |
| 9 | Hand or test-weight adapter, 6 × M3 into the flange inserts | From the hand side |

This check found two defects in the first option A layout, both fixed:

- **RS00 housing screws.** They sat on r 25, inside the Ø52.2 bearing bore and above the lip. No screw could reach them from below. The flange became the separate RS00 ring, which is bolted to the RS00 on the bench.
- **Wrist flange.** Its top stopped 1 mm short of the tube end, so its screws bridged a gap. Its head counterbores also started inside the part. Fixed with a spigot that seats on the tube end, and counterbores from the bottom face.

## FEA (5 kg sweep)

Load: 49 N plus 6 N self-weight at the hand point, 90 mm past the wrist face. Linear static. PETG: E 1.9 GPa, yield 45 MPa, layer strength 0.5 × yield. Print direction +X for the fork; the turned parts print axis-up. Targets:

- Sustained worst case: SF 1.5 bulk / 1.25 layer.
- Working case (500 g hand plus test weights): SF 2.0 / 1.5.
- Back-driven onto the elbow stop: 1.5 / 1.25.

### Sleeve, RS00 ring, wrist flange (2026-11-08, rebuilt geometry)

The sleeve and ring are modelled as one body, bonded at the clamp. They are fixed at the fork rim face and the 6 fork screw holes, with the load on the bearing bores. The turned parts print axis-up, so their layer criterion is tension across Y.

| Case | Sleeve + ring bulk / layer SF | Wrist flange bulk / layer SF |
| --- | --- | --- |
| Arm horizontal (P90) | 35 / 14.5 | 23 / 8.0 |
| Lateral | 40 / 16.4 | 23 / 8.2 |
| Hanging (P0) | 76 / 34 | 240 / 98 |
| RS00 14 N·m on the ring screws | 208 / 194 | — |

**Bearing seats under P90:** the lower seat moves 0.007 mm and the upper 0.001 mm. Ovality is 0.0008 mm or less. This model treats the fork as rigid; the fork's own deflection is in the fork case below.

**Screw loads** (hand calculation, centroid method, conservative against a heel pivot):

| Joint | Load per screw |
| --- | --- |
| Fork rim M3 | 93 N tension, 9 N shear. 72 N shear at the RS00 14 N·m peak if clamp friction is ignored |
| Flange to tube | 111 N tension |
| Hand inserts | 66 N tension |
| Wrist cap | 18 N, hanging |
| RS00 housing M3 × 6 | 93 N shear, torque only: about 10 MPa bearing stress in the ring |

All are well under heat-set insert pull-out in PETG.

### Elbow stop finger

The stop finger was checked at both ends of the elbow travel.

| End | Fork finger face | Carrier recess wall |
| --- | --- | --- |
| 0° | θ 23° | θ 23° |
| 124° | θ 63° | θ 187° |

- **Contact area:** both overlap areas are 35.8 mm² (X 140.5–146 × r 35–41.5), measured from the meshes. The centroid is at r 38.25.
- **Pressure at the RS02 peak:** 14.8 N·m gives 387 N, so the nominal pressure is **10.8 MPa**. The allowable is 55 / 1.5 = 36.7 MPa, so this passes 3.4 times over.
- **Pressure at the shoulder-driven case:** 34 N·m at the elbow, the gate 2 case, gives 889 N and 24.8 MPa. That also passes.
- **The FEA peak is a singularity.** The FEA reports −157 MPa at the finger root, inside the region for the rigid stop patch: it is compression at the fixture edge. The finger face and the carrier wall both stay as they are.

### Fork

Changed for draft 4:
- Loft knot at Y 155.5, so the belly meets both cheek outer faces nearly tangent. This removes a 28° kink and a zero-area sliver face at X 207.2.
- R4 inboard cheek-root fillet, Z −18 to 10.5.

Before the fix, the peaks sat on those sharp corners and grew with mesh refinement: P90 bulk SF went 3.23 → 2.95 → 1.84, and the stop-case layer SF was 1.01.

| Case, 5 kg (el = global mesh size) | Bulk SF | Layer SF (across X) | Peak location |
| --- | --- | --- | --- |
| Arm horizontal (P90), el 2.0 / 1.6 | 2.36 / 2.38 | 2.57 / 2.61 | Shelf corner at the outboard cheek (X 207.2, Y 153.7, Z 10) / outboard root (X 191, Y 158) |
| Lateral, el 2.0 | 7.61 | 4.03 | Inboard root |
| P90 back-driven onto the 0° stop, el 2.0 | 1.91 | 2.06 | Near the stop patch (X 138, Y 174) / inboard root |

The 1.6 mesh agrees with the 2.0 mesh within 2%. The 1.3 runs and the stop case at 1.6 are still running, and will be added to this table. All cases pass the sustained target (1.5 / 1.25) and the stop target (1.5 / 1.25).

## Open items

- **RS02 shoulder-to-output length** (from 2026-11-07, still open). Before the fork is printed, measure a real RS02 from the mounting shoulder to the output face.
  - The mounting shoulder is the flat ring with the nine M3 holes, where the Ø78.5 body steps down to the Ø65 back.
  - The output face is the raised Ø43 disc with the six M4 holes.
  - The vendor drawing gives 31.5–32.0 mm. The vendor 3D model gives 33.9 mm.
  - If the real part reads about 33.9, move the outboard cheek out about 2 mm. Details are in `../renders/elbow/README.md`.
- **Yaw limits disagree across the repo.** Not changed here, logged for the controls side:
  - Left URDF: ±1.05 rad.
  - `kinematics.md` left_wrist: ±1.6 rad.
  - Right arm: ±3.24 rad hard / ±1.55 rad soft.
  - `control.yaml`: [−0.367, 3.217] rad.

  The hardware stop in this design is ±170° (±2.97 rad). The clock-spring is sized for ±190°. One number needs to be picked and written everywhere.
- **Clock-spring detail design.** About 400 mm of reverse-wound FFC in the r 21.5–26.1 gap, Y 31–69. The end anchors and the ribbon part number are not chosen yet. A stop past ±180° would need a floating stop ring.
- **RS00 bearing is torque-only now.** RobStride publishes no output bearing rating (`hardware/docs/robstride/`). Option A takes the hand moment off it, so its rating no longer matters for the 5 kg cases.
- **Bearing seat fit coupon.** Print a Ø52.2 seat coupon before the sleeve, and adjust the bore to the printer.
- **Still to do:**
  - elbow ribbon loop and guard, and the forearm cable anchors;
  - RS00 fuse pocket and service cover;
  - print prep: 6 walls, 50% infill with 100% modifiers at the inserts and seats, tree supports, Super Tack, white PETG, 3MFs;
  - two-reviewer gate.

## Print orientation: one-piece fork vs clamshell (2026-11-08)

Joey's new rule: no support may touch a visible outer surface. The check is `support_check.py` in `../upper-arm/clamshell/`. It flags any face that is overhung more than 45° and not on the bed. A flagged face counts as outer only if a ray along its normal escapes the assembled arm (fork, RS02, carrier, upper arm, RS00, ring and sleeve).

| | One-piece fork B, best orientation | Clamshell: split on X 170.49 (the plane through the forearm axis), each half cut face down |
| --- | --- | --- |
| Support on the outer skin | **5800–7300 mm² in every orientation** (+X up: 6734) | **0 mm² for both halves** |
| Support on hidden faces | 6751 mm² | 6678 + 6255 mm². Mostly the cheek inner faces and the cavity roof, both open to the bed side and reachable |
| Mass, solid | 184 g | 92 + 92 g, plus a seam flange and inserts (about +8 g) |
| Layers | Normal to X | Still normal to X, so the layer criterion and the FEA above still apply. The seam lies along the bending stress, so it carries shear only |
| Cable | Back-wall channel 12 × 5 | The same back channel sits on the split plane, so the seam captures it. The cable runs from the elbow loop to the RS00 ring slot and is laid in before closing |
| Strength risk | Known (FEA above) | A new bolted seam in shear and twist (yaw torque 14 N·m). Needs the seam FEA |
| Print-quality risk | Rough outer skin | **The outboard cheek inner face (the RS02 output mating face) and the fork-stub bearing face print over support.** Hidden, but precision faces. Fix: print the cheek discs as separate flat parts bolted to the halves, or machine or sand to a coupon |

Recommendation: the clamshell, with the cheek discs as separate flat-printed parts. Images: `forearm_b_clamshell_concept.png`. The one-piece fork is not printed.

## Older drafts

Drafts 1–3 (the round Ø64 tube on the RS00 rim) are superseded. Their fold numbers and the failed fork FEA are in `../ELBOW_COMPACTION.md` and in git history.
