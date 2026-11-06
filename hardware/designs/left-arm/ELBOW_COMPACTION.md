# Elbow compaction

2026-11-06. Joey asked for a less bulky elbow, then left the choice to us with all validation passes kept. Implemented in upper arm rev C and forearm draft 3. Status: FEA, fold, roll and print checks done, gate review pending.

**Result:** the stack went from 78.5 to 75.5 mm, and the inboard face moved 5 mm toward the arm axis. Both bearings stay. The bearing now sits in the carrier instead of beside the fork, which removes the 9 mm air band. Section: `renders/elbow_compaction_study.png`.

## Stack along the elbow axis, arm coordinates

| Item | Before, X | Length | After, X | Length |
| --- | --- | --- | --- | --- |
| Inboard fork cheek | 122.7 to 132.7 | 10 | 127.7 to 135.7 | 8 |
| Air, stops and shoulder band | 132.7 to 141.7 | 9 | 135.7 to 136.5 | 0.8 |
| Carrier | 141.7 to 145.7 | 4 | 136.5 to 145.7 | 9.2, plus a floor to 147.2 inside the ring |
| 6808 bearing | 124.7 to 131.7, in the fork | 7 | 139 to 146, in the carrier | 7 |
| Steel ring | 145.7 to 147.7 | 2 | Same | 2 |
| RS02, nose inside the hoop | 147.74 to 193.14 | 45.4 | Same | 45.4 |
| Outboard fork cheek | 193.19 to 201.19 | 8 | 193.19 to 203.19 | 10 |
| **Total** | 122.7 to 201.19 | **78.5** | 127.7 to 203.19 | **75.5** |

| Measure | Before | After |
| --- | --- | --- |
| Stack centre from the arm axis (X 170.49) | 8.6 mm inboard | 5.0 mm inboard |
| Inboard overhang past the 64 mm forearm (X 138.5) | 15.8 mm | 10.8 mm |
| Largest diameter, hoop | 84.5 mm | Same |
| Fork disc radius | 29 mm | 29 mm, with a stop lobe to 41.5 mm |

Upper tube is 67 mm at its lower end, forearm 64 mm.

## Options

| Option | Saving | Cost | Verdict |
| --- | --- | --- | --- |
| A. Trim gaps and walls only | About 6 mm | Stops and M3 heads still need the band, so most of it comes back | Absorbed into B |
| B. Nest the bearing in the carrier, stops in a recess in the same plate | 8.2 mm of band removed, 3.0 mm net after the carrier grew 5.2 mm; cheeks trade 2 mm inboard for 2 mm outboard | Shorter bearing span; heavier fork | **Chosen** |
| C. Large thin-section bearing around the motor | About 11 mm | Web from tube to hoop blocks a full ring around the hoop, so it can only sit beside the hoop; diameter grows to about 98 mm; heavier bearing | Rejected |
| D. Recess the motor into a fork arm | Up to 2.4 mm (output step) | Thin cheek where the M4 load enters | Not taken |
| E. Single-sided fork, no second bearing | About 25 mm | RS02 output bearing takes all side load and moment; no RobStride rating, and protecting that bearing is a design rule | Rejected: does not win the checks |

Joey's offer of 1 to 2 mm on the outer faces was used: inboard cheek 6.5 went to 8, outboard 8 went to 10. At 6.5 mm the fork fell to SF 1.28 to 1.58 at the cheek root.

Moving the whole stack 4 mm outboard would centre it better. It needs the hoop, web, collar and fork sketches rebuilt, so it is left as a later option.

## Changes

- Carrier: Ø84, X 136.5 to 145.7, seat Ø52 to X 146, floor to X 147.2 inside the ring (Ø64).
- 6808 outer race held by the floor and 3 × M3 thread-forming button heads with Ø9 × 0.8 washers at r 29, in Ø9.4 pockets. Inner race floats on a fork stub, Ø40/Ø30.
- Stops: a 4 mm recess in the carrier face, r 34.5 to rim, θ 23 to 196 degrees. A finger on the fork runs in it, r 35 to 41.5, X to 140. Contact about 3.5 × 6.5 mm.
- Stop at **133 degrees, not 135**. At 135 the forearm rim is 3.9 mm from the tube, under the 5 mm target.
- Carrier M3 × 25 button heads sit in Ø6 counterbores on X 142.7, 6.5 mm engagement against 8 max.
- Fork M4 × 14 unchanged: the ring seat deepened 2 mm, so head and engagement stay at the same X.
- Collar root fillet stays at **1.5 mm**. The 3 mm fillet from gate 2 finding 13 ran into the M3 ligament and dropped LC1A from 2.18 to 1.69, so that finding needs a different fix.

## Validation

Same material, mesh and case set as `upper-arm/BASELINE.md`, rigid bolt model, 3 mm mesh, hoop-flat build direction.

### Upper arm

| Case | Rev B, same run today | Rev B, BASELINE | Rev C SF | Rev C layer SF |
| --- | --- | --- | --- | --- |
| LC1A | SF 2.02, layer 1.41 | 2.15, 1.36 | 2.18 | 1.33 |
| LC1B | | 2.15, 1.84 | 2.18 | 2.75 |
| LC1C | | 2.08, 2.13 | 2.14 | 2.06 |
| LC2 | | 2.47, 2.52 | 2.57 | 2.61 |

Worst SF improved. Worst layer SF is 1.33 against 1.36 to 1.41: **about 2 to 5 percent worse, and still under the 1.5 target**, at the same M3 hole edge on the RS02 side. With the steel ring also tying the hoop's inboard face (LC1A only), SF is 2.42 and layer 1.67. That is the as-built load path, not the baseline model. Deflection 1.00 mm fore-aft, 1.26 mm lateral. Combined cases and rev C jig predictions are not run.

### Forearm and fork

| Case | Previous draft | Draft 3 SF | Draft 3 layer SF |
| --- | --- | --- | --- |
| 100 N fore-aft | 2.23 to 2.26, layer 1.57 to 1.72 | 2.38 | 1.85 |
| 100 N lateral, 14 N·m twist | 2.23 to 2.26, layer 1.57 to 1.72 | **2.07** | 1.60 |

Twist case SF is **about 7 percent under the previous draft**, though above the brief's 2.0, and layer SF is 1.5 or better. The peak is at an outer gusset tip, a sharp corner. The hinge support moved to the stub, so the models are not identical. Tip deflection 2.9 to 3.3 mm.

### Load path

No metal added; carrier, stub and fork are printed, so the printed links stay the fuse. Inner race still floats, so axial load still goes only through the RS02 output.

| Load | Before | After |
| --- | --- | --- |
| Bearing span to RS02 output face | 64.9 mm | 50.6 mm |
| RS02 share of a load on the arm axis | 65% | 55% |
| Couple from a 19.5 N·m side moment | 300 N | 385 N, 28% up |

These are hand figures with the output face standing in for the output bearing. There are still no RobStride bearing ratings, so "arm breaks first" stays unproven by test.

### Clearance

| Check | Before | After |
| --- | --- | --- |
| Elbow stack to shell, 5 degrees inward roll, r 78 | 32.2 mm | 38.0 mm |
| Whole arm to shell, hanging, 5 degrees inward | 11.5 mm | 11.5 mm, at the tube top |
| Fork to arm, worst across travel to 133 | | 5.0 mm |
| RS00 to arm at 133 | | 10.0 mm |
| Cheek to carrier face, running gap | 0.5 mm | 0.8 mm nominal |

These are sampled point checks, 1 to 1.5 mm spacing, not solid interference. With the elbow bent and yaw at -90 the forearm passes through the torso, before and after: a joint limit job, not an elbow width issue.

### Print

| Part | Solid mass | Orientation | Findings |
| --- | --- | --- | --- |
| Carrier | 36.8 g | Outboard face up | 7.5 mm bridge over the recess; slivers where the pockets meet the seat |
| Fork | 181.5 g | +Y has least overhang, 2209 mm²; +X was the FEA direction, 11581 mm² | Feather edges at gusset tips, 0.06 to 0.96 mm, slicer will drop them |

Both fit the P2S bed. A Bambu Studio CLI slice of the fork ran end to end, but with its default profile and as-loaded orientation. It proves the mesh slices, not the print. **The fork orientation is open**: the layer figures above are for +X, and +Y is unstudied.

### Mass

| Part | Before | After | Delta |
| --- | --- | --- | --- |
| Carrier | 43.8 g | 36.8 g | -7.0 g |
| Fork with disc and finger | 168.4 g | 181.5 g | +13.1 g |
| **Total** | | | **+6.2 g** |

Solid PETG. Heavier because the fork lost the Ø52 hole and gained the stub.

## Files

Old revisions kept: `marengo_upper_arm_left_revC_wide78`, `marengo_forearm_left_draft2_wide78`. Scripts: `envelope/fork_fold.py`, `shell_gap_elbow.py`, `elbow_section.py`.
