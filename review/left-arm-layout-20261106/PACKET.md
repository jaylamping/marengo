# Left arm V1 master layout: adversarial review packet, 2026-11-05 (Joey-PC)

Status: draft layout, NOT released for printing, load, or power. Find what makes it fail.
SolidWorks binaries are hashed in SHA256SUMS.txt but not readable here; use views/, data/, scripts/.

## Definition
STEP coordinates: mm, Y up, X lateral, Z depth. Forward sign of Z is NOT established. Left arm = +X side (node 39); anatomical left is not independently confirmed.
Five joints, shoulder to wrist; same actuators as the right arm:
| # | Joint | Actuator | Axis (arm hanging) | Passes through (mm) |
|---|---|---|---|---|
| 1 | shoulder pitch | RS03, fixed in torso, output face X=97 | X | Y 424.86, Z -0.014 |
| 2 | shoulder roll | RS03, output +Z at Z 30.64 | Z | X 170.49, Y 424.86 |
| 3 | upper-arm yaw | RS02, output down at Y 319.86, clock 172.12 deg | Y | X 170.49, Z 5.986 |
| 4 | elbow pitch | RS02, output outboard (+X) at X 193.19, body centred on arm line | X | Y 209.86, Z 5.986 |
| 5 | lower-arm yaw | RS00, output down at Y 70.26, rear at Y 121.66 | Y | X 170.49, Z 5.986 |
Pitch and yaw axes are offset 6 mm in Z. Roll and yaw intersect; roll and pitch intersect.
Links (globals in the skeleton): L_UPPER 215, L_LOWER 195, L_HAND 90 mm, plus RS00_OFFSET 40 mm, measured down the yaw axis from Y 424.86. Wrist Y 14.86, stub end Y -75.14. The existing right arm was sized by eye (about 230 mm upper arm) and Joey feels it is already too long, so it is not a target. Lengths may shrink after visual review.
Reused unchanged from prior work: P1 L01, P2 L04B, printed 20 mm spacer, roll RS03 and yaw RS02 placements.
New and draft: elbow RS02, forearm RS00 positions. RS00 rear sits 88.2 mm below the elbow axis: 48.2 mm copied from the exported arm plus RS00_OFFSET 40 mm, chosen to clear a full-depth upper-arm shell at 135 deg. No upper-arm or forearm structure exists yet.

## Checks done
- Saved assembly closed and reopened from disk: load error 0, warning 0, 9 components, 0 suppressed, all fixed, each with a local file path (data/VERIFY.json).
- All four globals changed and restored, sketch segment lengths followed each change (data/VERIFY.json, global_test). Not saved.
- Component bounding boxes match the intended placement (data/component_boxes_mm.json).
- Source inputs hash unchanged after copying (data/INPUT_MANIFEST.json).
- Mass and torque hand-summed in scripts/05_mass_torque.py.

## Numbers
Mass 2373 g: RS03 880, RS02 405 x2, RS00 310 (supplied, not weighed); P1 169.8, P2 178.3, spacer 24.9 (CAD volume x PETG 1.27 g/cm3). COM (168.39, 328.15, 9.0).
Self-weight, arm horizontal, no links, hand, fasteners, cable or payload: pitch 2.26 N m at a 97.1 mm lever; roll 2.25 N m. Repo lists RS03 at 60 N m (peak). Each extra 100 g adds 0.11 N m at mid upper arm, 0.31 at mid forearm, 0.49 at the stub end.
Free gaps along the arm: yaw RS02 output to elbow RS02 top 70.75 mm; elbow RS02 bottom to RS00 rear 48.95 mm; RS00 output to wrist 55.4 mm.
Fold, computed by hand: at 135 deg flexion (45 deg between links) the RS00 rear inner corner sits 82.5 mm up the upper arm and 42.2 mm forward of the yaw axis, within the same X span, leaving 3 mm past a full-depth 39.25 mm half-depth shell. Before the offset it was 13.9 mm. Wrist to shoulder at that fold 158 mm.

## Not verified
No motion sweep, no interference check, no joint limits. No structure, fastening, or load path for the new links. Elbow bearing/side loading on a one-sided outboard output not assessed. Actuator masses not weighed. Motor outputs, connectors and cable routes through the arm unresolved (the left P2 route remains provisional). P1 pitch hole pattern shows a 7.83 mm nearest-hole residual at the export clock, unresolved. Output clocks not calibrated. Torso is simplified: no Pi, compute shelf or layout block; some motors are surface-only. Roll RS03 has 19 bodies after save, down from 23 in the STEP. Nothing is mated, so moving a global does not move the actuators, and the RS00 offset only moves a datum line in the skeleton until the part is re-placed.

## Questions
1. Joint order: does pitch, roll, yaw, elbow, lower-arm yaw with these offsets create singularities or workspace gaps that matter for a humanoid arm? What does the 6 mm pitch-to-yaw offset do?
2. Axis directions: is every axis and output direction right for the left arm, including elbow parallel to pitch and outboard output? What is wrong if forward is +Z versus -Z?
3. Link lengths: attack 215/195/90 against the torso in views/LEFT_ARM_LAYOUT_FRONT.png. Is it too long, is the forearm-to-upper ratio right, and what breaks if each is cut by 20 mm given the gaps above?
4. Elbow fold: can it reach past 135 deg with RS00 at this offset and a full-depth upper-arm shell? Is 3 mm enough, and what does the 55.4 mm left to the wrist cost?
5. Mirrorability: what stops this layout, or the parts it depends on, being mirrored to the right arm or back? Consider that actuators cannot be mirrored.
6. Anything else that makes it fail: give the failure, the evidence, and the smallest fix.

For each finding give severity (blocker, major, minor), evidence, and a fix. Do not praise. If something cannot be judged from this packet, say so.
