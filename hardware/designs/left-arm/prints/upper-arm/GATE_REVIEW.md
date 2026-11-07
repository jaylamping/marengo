# Upper-arm print package: gate 2/3 review (fa6d77d8)

2026-11-07. Package at fa6d77d8. Every reviewer worked read-only on a packet with SHA256s, and the hashes matched afterwards.

| Reviewer | Where | Time | Carrier | Upper arm | Cover | P2 L05 |
| --- | --- | --- | --- | --- | --- | --- |
| Codex Sol | Joey-PC, headless | 851 s | NO-GO | NO-GO | GO (fit trial) | GO (optional) |
| Codex Astra | Joey-PC, headless | 562 s | NO-GO | NO-GO | GO (fit article) | GO (optional) |
| Opus 5.5 high | cloud, fresh agent | about 16 min | NO-GO | NO-GO | GO with changes | GO with changes |
| Fable 5.1 high | cloud, fresh agent | about 25 min | NO-GO | NO-GO | GO | GO with note |

Not run: Muse Wonder and Argon. Run them on the fixed package.

**Decision: hold `carrier.3mf` and `upper_arm.3mf`.**

## Blockers (all four agree unless noted)

1. **The carrier ribbon slot is in the wrong place.**
   - It was cut at X 148.1–149.7, under an uncut spigot, instead of X 149.7–151.2.
   - That leaves a 0.6 mm membrane over the recess and fills the slot with support.
   - The slot dead-ends on the 6808 outer race.
   - The carrier FEA was run on the intended geometry, not this one.
   - **Action:** drop the in-bore B1 route and remove the slot. Joey has a card for a back-of-arm loop under a guard instead.
2. **LC1C at the hoop sleeve holes is a real mechanism, not a meshing artefact.**
   - The ligament from the Ø5 hole to the Ø66 bore is only 1.0 mm.
   - Moments go into the hole edges by sleeve pin bearing (Fable).
   - **Action:** a contact and preload joint model, with convergence shown through the ligament. Grow the ligament if it still misses.
3. **The hoop prints thicker than its sleeves** (Opus): faces off the layer grid, so 13.6 mm of PETG against 13.5 mm sleeves. The hub is line-to-line at 21.86 mm.
   - **Action:** an all-steel clamp stack, with the sleeves flush to +0.05 mm proud of the printed part after layer quantisation.
4. **The carrier is neither clamped nor piloted** (Opus and Fable). It bears on 1.0 mm counterbore walls with 0.2 mm clearance, and the Ø64 spigot is loose in the Ø66 ring.
5. **Every fit is zero clearance** with `xy_hole_compensation` 0: Ø5 and Ø6 sleeve holes, Ø66 bore, Ø52 seat (Fable). Add CAD clearance and confirm it with a fit coupon printed first.
6. **The port slot is 20 mm wide, not 23, and sits 3 mm off axis** (Opus, Fable). This no longer matters if the guard route is chosen.
7. **The "printed parts fail first" hierarchy is not shown** (Sol, Astra, Opus). In this model the arm's hole faces are weaker than the carrier stop.

## Majors

- **Fasteners:**
  - Hardware spec incomplete: screw torques, thread-locker, RS02 M3 engagement (5.0 mm), yaw-output M4 engagement (6.14 mm), and the depth of both threads.
  - Cover needs M3 heat-set inserts (Ø4 × 5 bosses).
- **Fuse pockets:** 3.5 mm deep × 30 mm against 35 mm holders, and the holder part is not defined. There is also a sliver void at the pocket back.
- **Ring:** the notched elbow ring is handed, so "either way up" is wrong. Moot if the slot is dropped.
- **Printing:**
  - Support inside the six Ø6 hub holes and the bottom bore notch, with no drilling step.
  - Upper-arm bed contact is only 15 mm².
  - The cover is about 27–43% support.
- **Sweep:** the carrier button heads sit about 1 mm proud in the fork-finger sweep, and the sweep has no screw heads in it.
- **Test plan:** pass and fail levels contradict each other (Astra). The packet doesn't define the LC1A/B/C/LC2 load cases.

## Overridden by Joey

Opus blocker 4 said rig tests 1–7 must pass before the upper arm is printed. **Joey chose (2026-11-07 01:12Z) to print the upper arm now.** The elbow rig test runs before the forearm is fitted and powered.

## Next

1. Fix the items above.
2. Rerun the carrier FEA and the LC1C joint model.
3. Re-slice and recheck with the same reviewers, plus Wonder and Argon if they're available.

## R4, recheck of a0c7d466 (Fable and Opus), 2026-11-07

No plate was NO-GO, and all 11 plates' settings, times and grams were verified. Changes made:

1. **Clamp tightening.** PETG takes almost all of the clamp's stretch, so 0.05 to 0.1 mm of creep empties it, and cured thread-locker would hide a loss at a 0.35 N·m retorque.
   - The hoop M3s go in dry.
   - Retorque at 1 hour, 1 day and 1 week with paint marks, and stop if any screw turns more than about 15 degrees.
   - Thread-locker goes on one screw at a time after the 1-week retorque.
   - A DIN 6796 M3 conical spring washer now sits on each DIN 9021. The carrier washer seat is 0.7 mm deeper, so the floor moves to X 149.75 and prints at 149.7, leaving a 2.0 mm web. Head top stays at X 146.65 or above, so the finger clears by at least 0.65 mm.
   - Screw: M3 × 25 cut to 24.0 gives 7.1 mm of thread into the 8 mm hole. Uncut would bottom.
   - `ASSEMBLY_ORDER.md` and rig test 8 match.
2. **Hub M4 bottoming.** Three DIN 125 washers per screw now, giving 5.8 mm of thread nominal and 6.6 mm worst case against the 7 mm depth. There's a step to measure each screw against its stack and add a fourth washer above 6.5 mm. Head room above the hub flat was checked by rays: clear.
3. `PRINT_ORDER.md` now says Ø4.3 for the hub holes.
4. **Root fillet.** The r 1.5 fillet is in the part and the exports, along the two concave edges where the hoop neck meets the tube wall at X 152 to 165. Rays show the blend from Y 254.86 to the wall over about 1.5 mm. What R4 saw was the *inboard* root corner at X 151.7: the 2.5 mm chamfer was only on −Z. It's now on +Z too, Z 13 to 39, so both sides match. `FEA_SUMMARY.md` now names both features.
5. **Dummy housing.** A 100% infill modifier now covers r 6 round each carrier screw from X 151.6 to 172.1, and the clamp face disc to X 154.8.
6. **`FEA_SUMMARY.md`.** The 38 N·m friction figure is now stated as a fresh-clamp upper bound, more like 12 to 30 N·m after relaxation. After a slip, the two-hole carrier and ligament bearing case is the bound. Screw bending into the RS02 isn't covered, so watch for it in rig test 5.
7. **`BUY_ORDER.md`.**
   - Now "fasteners, bearings and consumables".
   - DIN 6796 added. M3 × 25, DIN 9021, DIN 6796 and DIN 125 are bought as two sets, arm and rig, plus about 20%.
   - PTFE thrust washer marked optional, don't buy.
   - Paint marker added.
   - No HOLD note.
8. Bed contact is about 287 mm² in the first layer. The test-pass-1 note now says every plate has a brim.

The carrier FEA was rerun on the 2.0 mm web, at 17 N·m:

| Case | SF | Layer SF |
| --- | --- | --- |
| Stop 0, all six holes | 2.14 | 1.68 |
| Stop 124, all six holes | 1.84 | 1.35 |
| Stop 0, nearest two holes | 1.50 | 1.74 |
| Stop 124, nearest two holes | 1.61 | 1.18 |

The upper arm joint cases weren't rerun: the arm only gained the small +Z chamfer and the spring washer, and the hoop is unchanged.

