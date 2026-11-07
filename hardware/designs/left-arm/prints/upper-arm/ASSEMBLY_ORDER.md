# Left upper arm assembly order

**Screw torques.** The RS02 manual gives thread depths (9 × M3 ↓ 8 in the housing, 6 × M4 ↓ 7 minimum in the output) and says not to exceed them. It gives no torques, so these are my figures, not the vendor's:

| Screw | Torque | Preload, about | Thread in the RS02 | Check |
| --- | --- | --- | --- | --- |
| Hoop, M3 × 24, class 8.8 | 1.0 N·m | 1670 N, 57% of proof | 7.0 mm | Must not bottom in the 8 mm hole |
| Hub, M4 × 30, class 12.9 | 2.5 N·m | 3100 N | 6.14 mm | Must not bottom in the 7 mm hole |

Medium thread-locker on both.

1. **Coupons.** Print `fit_coupons.3mf` first and work through the table in `PRINT_ORDER.md`.
2. **Hoop and hub check.** All clamp load goes through steel, so the plastic must end up slightly thinner than the sleeves.
   - Sand the hoop inboard face flat, because it prints on support. Measure the hoop at each hole. Target 13.40, accept 13.35 to 13.45.
   - Fit a sleeve in each hole and check it stands 0 to 0.1 mm proud of the inboard face, with the other end flush on the seat. Use a straight edge and feeler gauge.
   - Check the hoop face on a flat plate: a 0.1 mm feeler must not pass under it.
   - Do the same at the hub: target 21.80, sleeve 0 to 0.1 proud.
   - **Sand the plastic, never the sleeves.** Don't over-sand: under 13.35 at the hoop, stop and tell me.
3. **Sleeves.** Fit the six hoop sleeves with retaining compound so it fills the gap, flush on the seat side. Wipe the ends clean. Fit the six hub sleeves in their Ø6.2 holes.
4. **Heat-set inserts.** Press the four M3 inserts into the cover bosses before anything else is in the tube.
5. **Bearing.** Press the 6808 into the carrier seat. Fit the three retainer screws and washers so they clamp the outer race.
6. **Elbow RS02.** Slide it into the bore until it stops on the sleeves at X 165.19. Fit the elbow plug through the port slot.
7. **Carrier.** Fit the steel ring, then the carrier with its pilot in the ring. Drop three DIN 433 washers into each of the six Ø6.05 holes. Fit the six M3 × 24 button heads with thread-locker and tighten to 1.0 N·m in a star pattern.
8. **Yaw RS02.** Put the hub ring on the flat at Y 298. Fit the six M4 × 30 hub screws through the sleeves into the yaw output, with thread-locker, to 2.5 N·m. Prove the tool access on a dummy first, because the top hub is tight.
9. **Trunk and fuses.** Run the trunk through the cover entry. Make the M14 splice near the shoulder end. Fit the two inline fuse holders in the rows on the wall opposite the window, held with hook and loop tape so they can be lifted out to the window to change a fuse. Leads leave each end along X, toward the tube walls. Check each joint through its range: no pull, pinch or rub.
10. **Cover.** Fit the cover with four M3 × 6 screws once the harness checks pass.
11. **P2.** If you made P2 L05 or filed the tip, the inward roll bench limit can go from 2 to 5 degrees, once you have checked 5 degrees by hand.

The elbow rig test comes next, before the forearm goes on. See `../../testing/ELBOW_DUMMY_OUTPUT_TEST.md`.
