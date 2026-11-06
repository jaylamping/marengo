# Gate review: left upper arm rev C and forearm draft 3, compact elbow, pass 2

Humanoid arm, printed PETG on a Bambu P2S with a 0.4 HF nozzle. The chain: pitch RS03 in the torso, roll RS03, upper-arm yaw RS02, elbow RS02, lower-arm yaw RS00. This gate covers the upper arm, its carrier, and the forearm fork around the elbow RS02. Print and bench test follow if it passes. Lengths 215/195/90 mm are provisional.

Design rules from the owner: aluminium frame and actuator output bearings must survive; cheap printed links are the fuse. Fall case is a ground-level tip-over. Human proportions where they cost nothing. Target SF 2 and layer SF 1.5.

## What is in here

- `data/ELBOW_COMPACTION.md`: the claims, the stack table, the options and all results. The "Pass 2" section is current; later sections record pass 1, with X positions 4 mm lower.
- `data/BASELINE.md`: load cases and rev A and B results.
- `data/marengo_upper_arm_left.step`, `data/marengo_forearm_left.step`: current geometry, arm coordinates, arm axis X 170.49, elbow axis along X at Y 209.861, Z 5.986. The upper arm STEP has three bodies: arm, carrier, cable cover.
- `scripts/`: fold, shell and section scripts, with STLs in `stl/`, `stl_comp75/` (pass 1), `stl_wide/` (before) and `marengo_forearm_left.stl`. `stl/` holds the pass 2 upper arm. Actuator placement comes from `data/component_boxes_mm.json`, now at pass 2. `fork_fold.py` and `elbow_section.py` have a hard-coded path for the fork STL on the author's PC; use the copy in `scripts/`.
- `views/`: section before and after, front, elbow fold, exploded, two FEA plots.

## Rev C hardware intent

- Hub: steel ring Ø32/ID16 × 2 on a flat at Y 298, Ø6 sleeves 21.86 long, DIN 912 M4 × 30, about 6.1 mm engagement in the RS02 output thread (manual: 7 mm minimum depth).
- Hoop: steel ring Ø84/Ø66 × 2 at X 149.7 to 151.7, Ø5 sleeves in the hoop, ISO 7380 M3 × 25 heads in Ø6 counterbores on X 146.7, 6.5 mm engagement against an 8 mm depth.
- Fork output: M4 × 14 through a Ø32 ring and 6 mm of PETG, 6 mm engagement.
- 6808 outer race in the carrier, held by the floor and 3 × M3 thread-forming button heads with Ø9 × 0.8 washers at r 29. Inner race floats on the fork stub.
- Fuse holders: 2 × 12.4 wide, 3.5 deep pockets on a pad inside the front wall, Y 260 to 290, reached through the cable window. ATM holder size is an assumption.

## Done

FEA, sampled-point fold and shell checks, solid interference across travel, fork print and thickness checks and a fork slice with the P2S PETG HF presets, all on pass 2, as listed in `ELBOW_COMPACTION.md`. Master layout and URDF updated, with the 133 degree limit.

## Not done

Interference check with screws, washers and the 6808, mesh convergence on pass 2, the fork as a component in the layout and URDF, solid modelling of the junction boards and elbow jumper (provisional, see `ELBOW_COMPACTION.md`), the SF cost of the forearm junction pocket and its cover opening (open, waits for the forearm pass), combined load cases, contact model of any joint, rev C jig predictions, any coupon, sliced print of the arm or carrier with the real profile, forearm fuse pocket, length decision. No RobStride bearing ratings exist. Material values are assumed.

## Questions

1. Is the bearing nested in the carrier sound: seat, floor, retainer, stub, inner race float, assembly order, and access to the 3 retainer screws and 6 M3 screws once the fork is on?
2. Do the stop recess and finger work: load path at 17 N·m peak, contact area, printed lug strength, and what fails first in a fall with the elbow at the stop?
3. Do the worst results survive scrutiny? Check the upper arm layer figure of 1.76 against pass 1 at 1.33, and the fork SF of 2.26 against 2.07. Why did they move, and are the model changes fair?
4. Check the load path table. Is the shorter bearing span the right trade, given the output bearing rule? Do the printed parts still fail before the actuator?
5. Check the stack table and the bolt lengths against the STEP files. Does any part collide, including heads and washers, across 0 to 133 degrees?
6. Is the fork and carrier printable as described? Is +X right for the fork, given +Y gives layer SF 0.82 and overhang favours +Y? Is there a better third orientation?
7. Is the option choice right? Is any option better? Does the 4 mm shift or the full-depth outboard gusset create a problem that pass 1 did not have, including the provisional wiring reservations?
8. What would make the test print a waste?
