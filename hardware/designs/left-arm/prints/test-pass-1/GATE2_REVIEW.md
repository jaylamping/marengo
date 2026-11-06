# Test pass 1: gate 2 (before print) review

2026-11-06. Packet: the six sliced 3MFs, `PRINT_ORDER.md`, `SETTINGS_CHECKLIST.md`, `../../testing/ELBOW_DUMMY_OUTPUT_TEST.md`, `../../BUY_LIST.md`, `../../MEASURED_MASSES.md`, `build_projects.py` (packet commit f2432ce, hashes checked by both reviewers).

**Reviewers:** Fable 5.1 high, about 28 min, and Opus 5.5 high, about 25 min. Both ran in the cloud as fresh agents with read-only access to the packet. Both checked the hashes, unzipped the 3MFs, measured the meshes, and parsed the embedded G-code.

**Not run:** Codex Sol and Astra, Muse Wonder and Antigravity Argon. Joey-PC was in use and these are test pieces, not arm parts. Run the full panel again at the gate for the forearm.

## Verdicts

| # | Plate | Fable | Opus | Decision |
| --- | --- | --- | --- | --- |
| 1 | PF03 roll spacer | GO | GO | **GO** |
| 2 | Layer coupons | GO with brim | GO with brim | **GO with an outer 5 mm brim set by hand** |
| 3 | Elbow carrier | GO with changes | GO as fit check | **GO as a fit-check piece only.** Reprint for the strength tests after the fixes below |
| 4 | Dummy output | GO with changes | GO with changes, hold for M8 stack | **Hold** |
| 5 | Fork sector | GO with changes | NO-GO | **Hold** |
| 6 | Dummy housing | GO with changes | GO with changes | **Hold** |

## Findings both reviewers agree on (verified from the files)

1. **No brim on any plate.** The setting is auto brim 5 mm, but auto brim added none on any of the six plates. The standing coupons (12 × 6 × 80 mm) and the two large solid parts need one.
2. **Fork sector has artefacts from the sector cut:**
   - an enclosed 44 mm³ void in the stub cheek (z 5 to 6.7);
   - a 0.24 mm skin closing the tube;
   - a 0.9 mm fin.
   The slicer also puts about 9 g of support inside the tube bore, where it can't be taken out.
3. **Stop travel is 123.4 to 124 degrees** (carrier opening about 164 degrees, minus the 40 degree finger). That matches Joey's 124 degree elbow limit. The test plan still said 133 degrees. **Fixed in the plan.**
4. **Test 7 pass criterion named aluminium threads** the fixture doesn't have. **Rewritten.**
5. **Settings:** all six project configs are byte-identical and match the checklist, with two differences:
   - The infill pattern is stored as `zig-zag`, which the GUI shows as Rectilinear. **Checklist wording fixed.**
   - The brim, see item 1.
6. **Dimensions and fits:**
   - The 6808 seat (Ø52.0) and the stub (Ø40.0) are modelled at zero clearance with no hole compensation. Expect a press fit, and sanding on the stub. The fit check records it.
   - The M4 × 16 is confirmed right.
   - The carrier, ring and housing hole patterns agree.
7. **Safety and test setup gaps:**
   - Lever distances were not tied to the elbow axis, which could put up to 33% more torque on the joint.
   - The mounting orientation was not stated: bolted flat to a bench, the axis is vertical and gives no torque.
   - There was no catch for to-failure tests.
   **All three are fixed in the plan.**

## Raised by one reviewer, accepted

- **Axial stack is not defined** (Fable). With a nut and washer under the dummy output, the fork sits about 1.8 mm short of the design position. That leaves either 0.7 mm interference at the carrier or 3.4 mm of bearing float, and in the float case the finger engages only about 3.6 of 5.4 mm. Opus found the related M8 × 45 problem: it likely hits the outboard cheek, and the nut stack has 0.2 to 0.5 mm clearance. **Fix:** define a spacer and the bolt length so the cheek-to-carrier gap is the design 0.9 mm.
- **The fixture doesn't test torque entering at the output flange** (Opus). In the lever tests the torque goes in through the stub, so the M4 and pin joint and the outboard cheek carry nothing. **Fix:**
  - Key the dummy output to the M8 (hex pocket).
  - Apply the 17 N·m case with a torque wrench through the stub bore.
  - Keep the lever for the external-load and limp-drop cases.
- **Fork support on functional faces** (Opus). Support fills the three Ø4 blind pin holes and sits under the cheek face that clamps to the output. **Fix:** flip the sector so the outboard cheek is down, or block support and drill the pin holes afterwards.
- **Carrier orientation** (Opus). The ring clamp face prints on support. **Fix:** flip it so the inboard face is down for the strength-test article.
- **Housing sleeve holes Ø5.04 are too tight for FDM** (Fable). **Fix:** open to Ø5.2 or ream. Infill on this fixture-only part can drop to about 40%, which saves about 6 h and reduces warp.
- **Dummy output pins** (Opus). Printed Ø3.8 pins are weaker than the real ones. **Fix:** use Ø4 steel dowels, and block support in the nut pockets.

## Open, not blocking this print set

- Finger to hoop-screw-head clearance is about 0.45 mm (inferred). Check it in the assembly.
- The Ø44 stub shoulder may touch the 6808-2RS seal. Check against the bearing drawing.
- The washer-in-hole slop (about 0.3 degree) uses most of the 0.5 degree stop-angle budget in test 4. Note it in the results.
- Side-moment numbers from test 7 won't compare directly with an RS02, because the dummy output pivots on an M8 thread.

## To do on Joey-PC (when it's free)

1. Re-cut the fork sector cleanly and flip it, or add blockers.
2. Define the M8 spacer and bolt length, and add the hex key on the dummy output.
3. Set the housing holes to Ø5.2 and its infill to 40%.
4. Flip the carrier test article.
5. Force a brim on every plate and re-slice.
6. Ask the two reviewers to recheck the changed files only.
