# Left upper arm assembly order

**Screw torques.** The RS02 manual gives thread depths (9 × M3 ↓ 8 in the housing, 6 × M4 ↓ 7 minimum in the output) and says not to exceed them. It gives no torques. These are my figures, set by the PETG under the washers, not by the screw.

| Screw | Torque | Preload, about | Pressure under the washer | Thread in the RS02 | Check |
| --- | --- | --- | --- | --- | --- |
| Hoop, M3 × 25 cut to 24, DIN 6796 over DIN 9021 | **0.35 N·m** | 580 N | 10 MPa on 55.6 mm² | 7.1 mm | Must not bottom in the 8 mm hole |
| Hub, M4 × 30, three DIN 125 | **0.6 N·m** | 750 N | 15 MPa on 48.4 mm² | 5.8 mm nominal, 6.6 worst case | Must not bottom in the 7 mm hole |

**These are low on purpose.** The clamp goes straight through the plastic, and PETG takes almost all of the clamp's stretch, so 0.05 to 0.1 mm of creep would empty it. The DIN 6796 conical spring washer under each hoop screw stores travel to cover that. Use a torque screwdriver.

**Fit the screws dry at first.** Cured thread-locker breaks away at about 0.5 N·m, which would hide any preload loss at a 0.35 N·m retorque. Thread-locker goes on only after the 1-week retorque, step 13.

## Order

1. **Bearing fit.** Print `seat_ring_only.3mf` and press the 6808 into the seat by hand, then try it on the Ø40 stub. Firm hand press, no rock. If it's too tight or loose, tell me before you print the carrier. The 6808 has never been fitted to a print.
2. **Hoop and carrier faces.** The hoop inboard face prints on support. Sand it flat on a plate so the carrier beds on it. Don't go under 13.3 thick. The carrier outboard face prints up, so it only needs a wipe.
3. **Holes.** Clear the Ø3.3 hoop holes and Ø4.3 hub holes of support with a drill held in your fingers.
4. **Heat-set inserts.** Press the four M3 inserts into the cover bosses before anything else is in the tube.
5. **Bearing.** Press the 6808 into the carrier seat. Fit the three retainer screws and washers so they clamp the outer race.
6. **Elbow RS02.** Slide it into the bore until its shoulder sits on the seat at X 165.19. Fit the elbow plug through the port slot.
7. **Carrier.** Put the carrier on the hoop inboard face. Put a DIN 9021 washer in each of the six Ø9.4 seats, then a DIN 6796 on top with its cone up, toward the head. Fit the six M3 × 24 button heads **dry**, and tighten to 0.35 N·m in a star pattern, in two passes. Check that the finger clears every head by hand.
8. **Yaw RS02.** Put three DIN 125 M4 washers under each of the six M4 × 30 hub screws. **Before fitting, measure each screw against its stack:** screw length, minus the three washers, minus the hub thickness at that hole. If any gives more than 6.5 mm of thread, add a fourth washer to that screw. The output holes are 7 mm deep. Fit them dry to 0.6 N·m. Prove the tool access on a dummy first, because the top hub is tight.
9. **Trunk and fuses.** Run the trunk through the cover entry. Make the M14 splice near the shoulder end. Fit the two inline fuse holders in the rows on the wall opposite the window, held with hook and loop tape so they can be lifted out to the window to change a fuse. Leads leave each end along X, toward the tube walls. Check each joint through its range: no pull, pinch or rub.
10. **Cover.** Fit the cover with four M3 × 6 screws once the harness checks pass.
11. **P2.** If you made P2 L05 or filed the tip, the inward roll bench limit can go from 2 to 5 degrees, once you have checked 5 degrees by hand.
12. **Retorque.** PETG relaxes under a clamp. Paint-mark each hoop and hub screw head across to the part after the first tightening. Retorque every one to the table value at 1 hour, 1 day and 1 week, and re-mark each time. **If any screw turns more than about 15 degrees at the 1-day or 1-week retorque, stop and tell me.**
13. **Thread-locker.** After the 1-week retorque, take each screw out one at a time, put medium thread-locker on the tip threads only, so none reaches bare PETG, and refit it to the table torque before moving to the next.
14. **Spin check.** With the fork on, turn the elbow by hand through its range. It should be smooth, with no rub at the cheek. See `../../MOVING_INTERFACES.md`.

The elbow rig test comes next, before the forearm goes on. See `../../testing/ELBOW_DUMMY_OUTPUT_TEST.md`.
