## Verdict

Not ready to cut or order. The frame geometry itself checks out (cut lengths, bracket count, fastener sums, gusset DXF and mass all reproduce), but three things would make the order a waste or the packaging fail: the pack has no positive retention, the pack tray and board carrier have no load path to the frame, and there is no cable or air path from the lower box to the upper bay. Four shelf-level brackets also overlap the compute shelf and shelf rails in the generator's own data, so claim 6 ("no interference") is false.

**Verified** (from `scripts/frame.py`, `data/parts.json` records, `gusset_plate_left.dxf`, `bom.json`, the photos): all coordinates quoted below, the cut list (3124 mm, 1024 new), 32 brackets, 64+36+20+8 = 128 M5/T-nuts, gusset shoelace area ≈5.2e3 mm² gross → ~40 g, DXF R15 at the X113/Y335 corner (arc centre 98, 340.05), BOM sums, 17 g tip-over arithmetic, single-rail built frame with printed corner gussets at the front and two-hole metal plates at the rear.
**Assumed**: button-head height 4.5 mm and bracket leg 4 mm (the generator's own values); PETG E ≈ 2 GPa; N52 6×3 magnet pull ≈ 10 N each; RS03 housings at system GND (the README assumes this too). I could not run scripts in this session; every interference below was computed by hand from the generator's coordinates.

## Findings

**Blockers**

1. **Pack retention backward is magnets only.** 2.2 kg × 17 g ≈ 0.37 kN. D2 holds with 3 to 8 magnets ≈ 30 to 80 N. The pack travels ~44 mm free (rear face Z −45, door inner ≈ Z −89) then arrives at the door with its own 16 J. Forward it loads the 4 mm PETG carrier through 3 mm of air; a 0.37 kN uniform load bows a 100-wide 4 mm PETG plate ≈3 mm, which closes the 2 mm gap to the board's rear face. Up/down/side on the rails are fine (≈5 MPa in the lips). Smallest fix: a removable rear stop between the two tray rails at Z −50 (M5 cross-bar or two M3 thumbscrews into case bosses) plus a 6 mm front lip on the rails over the pack's front edge. Door stays cosmetic.
2. **Tray rails and carrier float.** `packrail_±1` occupy X 47.5–55, Z −50..40; `pdb_carrier` X ±50, Z 37.5–41.5. The lower posts are at Z ±50..70, so neither part touches a post face ("on the lower post inner faces" is wrong; the rail only corner-touches the rear post at X 55, Z −50). No mounting hardware for either is in the BOM or T-nut count. Fix: flanges onto the posts' inner X faces at X 51–55, Z 50–62, Y 203–325 (free between the corner brackets), 2 T-nuts each, and add them to the nut map.
3. **No path from lower box to upper bay.** At the step: shelf plate X ±55, shelf rails X 55–75, pitch bracket caps X 78.4–93 (Y 373.62+, Z ±50), posts X 93–113. Only a 3.4 mm sliver at X 75–78.4 remains. `harness_step` (X 33–45, Z 50–60, Y 340–376) overlaps `rail_shared_1` (Z 50–70, Y 353.62–373.62); `harness_down_±1` (X 47–55, Z 62–70, Y 160–370) overlaps the shared front rail and the two front bracket legs at X 51–55 (Y 175–203 and 325.62–353.62). The same wall kills the cooling story (finding 9). Fix: reserve the trunk and LV route outside the frame face, Z 73–85 at X ±55..75 over the gusset plates (shell front gap is 43 mm at H3), or slot the shelf plate.

**Major**

4. **Shelf plate vs four shelf-rail brackets.** `br24/26/28/30` legs (e.g. X 27–55 × Y 353.62–373.62 × Z 46–50 and X 51–55 × Z 22–50) sit inside `shelf` (X ±55, Y 354.6–372.6, Z ±50); real screw heads add 4.5 mm more. Fix: L-notch the plate corners 32 × 32, or drop the brackets and end-tap the shelf rails M5, bolting through the shared rail with the gusset bolt (move gusset hole X 62 → 65).
5. **Step-side 2020 brackets still 2 mm into the shelf rails.** `br25/27/29/31` leg X 73–93 vs `rail_shelf_1` X 55–75; the gap is 18 mm, the leg 20. Fix: end-tap the step side rail through the shared rail at the gusset hole (move X 104 → 103), or a flat two-hole plate under the step.
6. **Pack removal path hits screw heads.** Rear brackets' vertical-leg heads protrude to X ±46.5 at Y 184.75–193.25 and 335.4–343.9, Z −64..−56; the pack is X ±47.5. Top heads clear the pack by 0.12 mm (Y 345.12 vs 345), bottom heads overlap by 0.5 mm (Y 183.5 vs 183). The "8 mm top and bottom" margin is ~0 at the rear corners. Fix: replace the four lower-post-top X-face brackets with end anchors (finding 7), or make the pack 92 wide.
7. **Step joint.** The pack adds ~6 % to the lower-body load (≈0.86 → 0.92 kN per bolt), not materially worse, but friction with SF ≈1.1 on relaxing T-nuts is the real problem. The anchor needs no gusset change: tap each post end M5, access hole in the shared rail top at X ±65 / underside at X ±103, Z ±60 (all free faces). Gussets can be ordered once findings 4–5 fix the two shared-rail hole positions.
8. **parts.json fastener geometry is wrong for all 64 bracket fasteners.** `bolt_bracket` puts heads inside the leg (`frame.py:177`) and T-nuts 4 mm shallow (`:182`; e.g. `t0_1` X 52.5–58.5 against a post face at X 55). Any clearance claim derived from it is unverified.
9. **Cooling path does not exist as described.** The lower box is closed below by the waist adapter plate, above by the step (finding 3), behind by pack and door. The two intakes blow onto the board face and can exit only through 5 mm bands at X 50–55 (with pigtails in them) to the open side faces, which the shell designates as V1 *intakes*. D1 exhausts cannot pull from the lower box. Worst hot spot: the HP BMS banks (Y 294–318, only 10 mm under the upper intake's Y 264–304) and the HP rear face in dead air 9.5 mm from the pack lid. Fix: make V1 the lower-box exhaust, or open the shelf plate.
10. **Brake resistor** (X −55..−47, Y 155–180, Z ±25) sits in the sealed bottom pocket, touching the PETG tray rail at Y 180 and 3 mm under the bottom cells, cooled by one 100 mm rail. The supervisor's 40 W average fault limit is ~4× what that pocket sheds. Fix: move it to the −X lower post outer face, Y 285–335 (side gap, V1 airflow); fault at ≤10 W average. Docs disagree on its location (`frame.py:237` comment and `pdb_PLAN.md` say front bottom rail).
11. **Pack doc still specifies a 32 V MIDI fuse** (`custom-21700-pack.md` §4) for a 54.6 V pack; the plan says Class T. The superseding `pack-build-plan.md` is not in the packet. Order from the wrong table and the fuse arcs.
12. **Pi rear ports** reach Z −71.8 inside a 10–17 mm shell gap; the LP→Pi USB and CAN plugs need ~25 mm. Worse than stated: not serviceable with D1 closed. Right-angle plugs or rotate the Pi.
13. **Waist RS03 decision can scrap the posts.** If the hip answer is a taller lower box rather than a bolt-on ring, the 198.62 lower posts are waste and the 500 mm sticks cannot re-yield 340s. Decide before cutting.

**Minor**

- `harness_shoulder_±1` overlaps `jetson` (X 42–51.5, Y 447–455, Z −32..−22); the flat-A keep-out overlaps the Jetson by 1.5 mm in X and 15 mm in Y, so "2 mm above the Jetson top" is wrong. The Jetson shelf has nothing to mount to: pitch brackets cover the upper posts' inner faces; only standoffs from the shelf plate at X ±45 or the top rails.
- `pdb_carrier`/`pdb_sheet` overlap `packrail_±1` by 2.5 mm in X.
- Spine pad envelopes start at Z −77: gusset heads reach Z −77.5, H2 brackets Z −82 (overlap X 68–75, Y 475–481). Pads start inside the shell wall.
- H2 hardpoint heads reach Z −86.5 against a 15.3 mm gap (Z −85.3): counterbore the PETG.
- Harness over the pack lid in 8.6 mm under a bare T-slot lip: chafe risk on PACK+. Slot cover strip.
- Doc arithmetic: frame-only mass is 2.47 kg by its own BOM, not 2.1; README §7 says 28+4 brackets vs §2's 24+8; §2 table says M5×16, shopping says ×20; item 27 is 5/220 g in the README, 8/270 g in bom.json; FEA README says "40 g total" gussets.
- T-nut count omits tray rails, carrier, Jetson shelf, resistor (M3), clips, pack stop: short by 15–20.
- Photos show no cast 2028s on the bottom ring, so "16 reused, 14 spare" is an inventory guess.
- Frame bond: the 1 MΩ ∥ 10 nF point is moot if the RS03 housings are at GND (assumed, not measured). Then a PACK+ lug or chafed lead to the frame shorts the pack through an actuator's internal ground bond before the 60 A Class T opens. Measure housing-to-GND on one RS03; boot the lugs.

## Answers

1. Retention: backward fails (finding 1); forward loads the carrier; side/up/down OK once the rails have flanges (finding 2). Rear stop + front lip is the smallest fix.
2. Crash shorts: pack-to-board is PETG-separated and fine; the risks are the 10 AWG pair on the slot lip and bare M4 lugs 8.6–13.6 mm under the rail. The bond does not make it worse; the actuator housings make it irrelevant (minor list).
3. Overlaps: findings 3, 4, 5, 6, 8 and the minor list; near-misses under 2 mm: RS03 body to top side rail 0.64, to posts 0.5; packrail to bracket 1.62; Pi to H3 rear 2.0; H5 to spine pad 2.0.
4. Pack: unplug XT90 and Micro-Fit at D2, slide; catches on four screw heads (finding 6); over-the-pack harness must be free at the pack end. Boards: remove U3 and both fans, unplug XT90 first (HP terminals are live), undo ~10 pigtails, 6 M3, then the undefined carrier mounts, and the 100×150×24 carrier exits the 110×178 front opening.
5. Not plausible (finding 9). Worst spot: BMS FET banks and the HP rear face; the resistor pocket (finding 10).
6. Cut plan and lengths are right; upper post 121.38 is pinned by the pitch bracket ear slots (363.69 and 485). Missing on the bench: tray rail and carrier hardware, resistor M3 T-nuts, pack stop, ~20 more T-nuts, an M5 tap and drill if end anchors are adopted, a verified 2020 bracket with ≤18 mm legs, and the real bracket inventory.
7. Marginally worse (≈6 %); the anchor is independent of the plate; order gussets after findings 4–5 settle two hole positions.
8. The waist decision (finding 13); the gusset hole pattern (findings 4, 5); a pack wider than 95 after real holders are measured only costs rails, not posts.
