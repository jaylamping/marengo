# Left forearm and elbow fork, draft 3

**Superseded in part by draft 3, see `../ELBOW_COMPACTION.md`.** Fork width is now 75.5 mm at X 131.7 to 207.19, mass 171.1 g at 1.19 g/cm³, stop 133 degrees, print direction +X, and the 6808 sits in the carrier on a fork stub. The bearing notes below describe the older layout.

2026-11-06. **Not released, not reviewed.** Built at the 215/195 skeleton, and rebuilds with the length decision. The CAD stays on Joey-PC. Coordinates are arm coordinates, with the arm axis at X 170.49. The master layout moves it +38 mm, the same as the upper arm. It is not in the layout or URDF yet.

- Fork: outboard cheek bolts to the elbow RS02 output with 6 M4 on a 24 mm circle. Inboard cheek holds a 6808 bearing on the carrier stub.
- Tube: round, Ø64 with a 3 mm wall, around the RS00. It bolts to the RS00 rim with 6 M3 on a 50 mm circle from below.
- Mass: 161 g, solid PETG.
- Width across the fork: 80 mm.

## Open item, 2026-11-07: RS02 shoulder-to-output length

Before the forearm is printed, measure a real RS02 from the mounting shoulder (the flat ring with the nine M3 holes, where the Ø78.5 body steps down to the Ø65 back) to the output face (the raised Ø43 disc with the six M4 holes). The vendor drawing gives 31.5 to 32.0 mm, and the hoop seat to fork cheek gap is 32.0 mm. The vendor 3D model gives 33.9 mm. If the real part reads about 33.9, move the fork's outboard cheek out about 2 mm. Details in `../renders/elbow/README.md`.

## Changes needed on the upper arm

- **Stub moves inboard.** The current stub puts the bearing at X 133.7 to 140.7, which fills the cable band. The fork assumes a stub to X 124.7, with the bearing at X 124.7 to 131.7. That leaves X 132.7 to 143.7 free.
- **Stops move.** The carrier lugs need a stop finger on the inboard cheek inside that band. Neither exists yet.
- Inboard reach grows from X 131 to X 122.7, or 160.7 in the master.

## Elbow travel

`fork_fold.py`, sampled every 15°.

| Elbow | Fork to upper arm, mm | RS00 to upper arm, mm |
| --- | --- | --- |
| 0 to 60° | 11.5 | 50 |
| 90° | 9.6 | 50 |
| 105° | 7.1 | 47 |
| 120° | 3.4 | 29 |
| 135° | 2.8 | 7 |

The front edge is cut back near the elbow so the fork folds past the tube. The last 15° is short of 5 mm, so trim the bridge front or stop at 130°.

## FEA

Linear static, PETG as in the upper arm baseline, layers across the fork. M4 holes fixed, bearing seat as a hinge, M3 holes loaded rigidly. The same limits on that model apply as in `../upper-arm/GATE2_REVIEW.md`.

| Case at the hand, Y 20 | Peak, MPa | Safety factor | Layer safety factor | Tip deflection, mm |
| --- | --- | --- | --- | --- |
| 100 N fore-aft, about the 17 N·m elbow peak | 18.6 | 2.4 | 1.9 | 2.5 |
| 100 N lateral + 14 N·m RS00 torque, before the last gusset trim | 14.9 | 3.0 | 1.8 | 1.9 |

## Still to do

- Steel ring at the M4 heads, as on the rev C hub.
- Bearing fit and retention: a 2 mm lip on the outer side, nothing yet on the inner race.
- Fuse pocket for the RS00 drop, about 30 × 12 × 8 mm, through a service opening.
- Cable route through the fork, clamp points, print orientation and supports.
- Hand interface: the 6 M3 output pattern is reached through the Ø40 ring.
- Tip-over and fuse check.
