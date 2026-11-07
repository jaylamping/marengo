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
