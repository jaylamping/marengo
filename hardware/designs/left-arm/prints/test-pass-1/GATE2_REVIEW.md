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

## Recheck R2, 2026-11-06

Opus about 15 minutes and Fable about 10 minutes, on the changed files only.

| Plate | Verdict at R2 |
| --- | --- |
| 1 to 4, and the housing | GO |
| Dummy output | GO with changes |
| Fork sector | NO-GO until the slit is filled |

What changed after R2:

- **Fork sector, slit:** a 1.0 mm slit at Y 141 to 142 between the tube and the crown. Fill extended to Y 142.6.
- **Fork, cheek slots:** a gap of up to 2 mm where the transition top meets the yoke, in both cheeks. It came from the live fork, so the same gap was in the real part. New feature `CHEEK_ROOT` in `marengo_forearm_left.SLDPRT` follows the transition outline from Y 149.9 to Y 152.6. Live part and exports updated, and the sector rebuilt from it.
- **Fork sector, crown:** the transition cavity is now suppressed in the build script as well, so the sector rebuilds the same way.
- **Dummy output, wall:** only 1.2 mm between the M8 head pocket and the M4 nut pockets. The disc is now 13.19 mm thick. The nuts slide in from the rim at X 189.7 to 193, and the head sits behind them at X 184 to 189.4, with 9.8 mm of wall round the corners. Dowel holes are 4.2 mm deep, to match the 4 mm in the fork. M4 holes go through, so the M4 × 16 tips have room.
- **M8:** with the head moved 6 mm toward the housing, M8 × 55 now gives the thread length that M8 × 65 would have. Spacer 4.0 mm. Class 10.9, because 17 N·m is 320 MPa in shear at the thread root, and class 8.8 yields at about 370 MPa.
- **Nuts:** no cross-pin, because a Ø3 hole through the thread would leave the bolt too weak at 17 N·m. Instead thread-locker, 30 N·m between the nuts, a socket on both nuts and a paint line, with tightening direction only as the fallback. Wrench limited to 25 N·m.
- **Notes:** added cleaning of the strength carrier ledge, pockets and stop walls, scraping the carrier lip, drilling the fork dowel holes, sanding the stub cheek face and Ø32 seat floor before the feeler check, and retorquing the leg bolts.

Plates 5 and 7 are re-sliced. The other five are unchanged. The fork strength numbers are still open, see the forearm notes.

## Recheck R3 (fork sector and dummy output only)

Fable about 6 min, Opus about 10 min, same method. **Both plates GO.**

- Fork: slit and cheek slots closed. The old slit plane is a full 3179 mm² section, against 470 mm² before. One solid body. A small open notch, about 2 × 3 × 4 mm, remains at the stub-cheek root (x 128–130, y 124–126, z 69–73 print frame). It is in the live fork too, so the test stays representative. Fill it in the live fork later, and note it if the first crack starts there.
- Dummy output: the thin wall is gone. Hex pocket to nut slot is at least 1.19 mm in plan, at different heights. Axial stack confirmed: 0.8 mm running gap, M4 × 16 still right, dowels fit, bolt tip clear, second nut fully on thread.
- Changes made from R3, in the docs:
  - Jam torque cut from 30 to 15 N·m, and the wrench limit from 25 to 20 N·m. Fable found that jam preload plus torsion put the bolt over class 10.9 yield at 25 N·m (von Mises about 1000 MPa).
  - The bolt is now specified fully threaded (ISO 4017), so the first nut is on full thread.
  - Clean and test-fit the M4 nut slots, which the slicer fills with support.
  - Drill the fork dowel holes about 4.5 mm deep.

**After R3:** the notch was the same cheek-root slot (x 134–138, z 7–10, both cheeks). It is now closed in the live fork by a second `CHEEK_ROOT` change. The pushed `fork_sector.3mf` (54b06c930b81, the R3 GO version) still has it and is kept as is. That makes the stop test slightly conservative. The next forearm export replaces it.
