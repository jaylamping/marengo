# Left upper arm assembly order

**Screw torques.** The RS02 manual gives thread depths (9 × M3 ↓ 8 in the housing, 6 × M4 ↓ 7 minimum in the output) and says not to exceed them. It gives no torques. These are my figures, set by the PETG under the washers, not by the screw.

| Screw | Torque | Preload, about | Pressure under the washer | Thread in the RS02 | Check |
| --- | --- | --- | --- | --- | --- |
| Hoop, M3 × 24 | **0.35 N·m** | 580 N | 10 MPa on 55.6 mm² | 7.2 mm | Must not bottom in the 8 mm hole |
| Hub, M4 × 30 | **0.6 N·m** | 750 N | 15 MPa on 48.4 mm² | 6.6 mm | Must not bottom in the 7 mm hole |

Medium thread-locker on both, on the screw tip only, so that none reaches bare PETG. **These are low on purpose.** The clamp goes straight through the plastic, and more torque makes it creep. Use a torque screwdriver.

## Order

1. **Bearing fit.** Print `seat_ring_only.3mf` and press the 6808 into the seat by hand, then try it on the Ø40 stub. Firm hand press, no rock. If it's too tight or loose, tell me before you print the carrier. The 6808 has never been fitted to a print.
2. **Hoop and carrier faces.** The hoop inboard face prints on support. Sand it flat on a plate so the carrier beds on it. Don't go under 13.3 thick. The carrier outboard face prints up, so it only needs a wipe.
3. **Holes.** Clear the Ø3.3 hoop holes and Ø4.3 hub holes of support with a drill held in your fingers.
4. **Heat-set inserts.** Press the four M3 inserts into the cover bosses before anything else is in the tube.
5. **Bearing.** Press the 6808 into the carrier seat. Fit the three retainer screws and washers so they clamp the outer race.
6. **Elbow RS02.** Slide it into the bore until its shoulder sits on the seat at X 165.19. Fit the elbow plug through the port slot.
7. **Carrier.** Put the carrier on the hoop inboard face. Put a DIN 9021 washer in each of the six Ø9.4 seats. Fit the six M3 × 24 button heads with thread-locker, and tighten to 0.35 N·m in a star pattern, in two passes.
8. **Yaw RS02.** Put two DIN 125 M4 washers under each of the six M4 × 30 hub screws. Fit them through the top hub into the yaw output with thread-locker, to 0.6 N·m. Prove the tool access on a dummy first, because the top hub is tight.
9. **Trunk and fuses.** Run the trunk through the cover entry. Make the M14 splice near the shoulder end. Fit the two inline fuse holders in the rows on the wall opposite the window, held with hook and loop tape so they can be lifted out to the window to change a fuse. Leads leave each end along X, toward the tube walls. Check each joint through its range: no pull, pinch or rub.
10. **Cover.** Fit the cover with four M3 × 6 screws once the harness checks pass.
11. **P2.** If you made P2 L05 or filed the tip, the inward roll bench limit can go from 2 to 5 degrees, once you have checked 5 degrees by hand.
12. **Retorque.** PETG relaxes under a clamp. Retorque every hoop and hub screw to the table value after the first hour of running, and again after the first day. Paint-mark each head after the second retorque. If a screw turns more than a quarter turn at the second retorque, stop and tell me.
13. **Spin check.** With the fork on, turn the elbow by hand through its range. It should be smooth, with no rub at the cheek. See `../../MOVING_INTERFACES.md`.

The elbow rig test comes next, before the forearm goes on. See `../../testing/ELBOW_DUMMY_OUTPUT_TEST.md`.
