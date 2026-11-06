# Test pass 1 print order

These are test pieces, not load-bearing arm parts. Don't fit them to the robot, apart from the PF03 spacer. Settings: `SETTINGS_CHECKLIST.md`. Tests: `../../testing/ELBOW_DUMMY_OUTPUT_TEST.md`.

| Order | File | Print time | Filament | Down on the plate | Why |
| --- | --- | --- | --- | --- | --- |
| 1 | `PF03_roll_spacer.3mf` | 1 h 1 m | 23.7 g | Axis vertical | Reprint at 100% infill. The last one weighed 11 g |
| 2 | `layer_coupons.3mf` | 2 h 43 m | 70.3 g | 5 bars standing, 5 flat | Real layer strength, test 1 |
| 3 | `elbow_carrier.3mf` | 1 h 9 m | 29.5 g | Outboard face | Fit check only, with the real 6808 and rings, tests 2 and 3 |
| 4 | `elbow_carrier_strength.3mf` | 1 h 15 m | 32.8 g | Inboard face | The carrier for the stop tests |
| 5 | `dummy_output.3mf` | 35 m | 9.7 g | Cheek face, spacer beside it | Output disc and the M8 spacer |
| 6 | `dummy_housing.3mf` | 6 h 31 m | 233.0 g | Carrier face | Stands in for the RS02 and upper arm. 40% infill |
| 7 | `fork_sector.3mf` | 9 h 2 m | 340.7 g | Outboard cheek | Stop and failure tests |

Total: about 22 h 36 m and 740 g, including support and brim.

**Gate 2 review, 2026-11-06 (`GATE2_REVIEW.md`):** the fixes it listed are made in these files. Plates 1 to 3 were GO. The changed plates 4 to 7 still need the reviewers' recheck before printing.

**Stop after step 2 or 3 if something is off.** Weak coupons change every layer number, and a bad 6808 fit means the CAD changes before the bigger prints. Steps 6 and 7 take most of the plastic, so don't start them until steps 2 and 3 look right.

Tests 5 and 6 break parts. If the first set goes well, print a second carrier and fork sector.

## After printing

- **Coupons:** remove the brim gently, and don't flex the standing bars.
- **Carrier:** clear all support from the recess, counterbores and bearing seat. Check the bridge over the recess. The strength carrier has more support, in the recess, so check the stop walls are clean.
- **Dummy output:** clear the small support from the three dowel holes with a Ø4 drill by hand, then press in the dowels. Press the M4 nuts into the six pockets. The pockets print open, with no support.
- **Fork sector:** has a lot of support under the inboard cheek, the finger and the stub. Take it out in small pieces, and don't pry against the stub, finger or cheek faces. The Ø32 seat is against the bed, so its floor prints on support. Sand it flat before fitting the ring.
- **Dummy housing:** press M3 nuts into the six hex pockets from the back. Sleeve holes are Ø5.2.

## M8 stack

This sets the fork at its design position, with the 0.8 to 0.9 mm running gap at the carrier and the finger in full contact.

From the output outward: M8 × 55 hex head bolt with its head in the output's hex pocket, then the printed spacer (10.0 mm, measure it) between the output back and the housing core, then the housing bore, then a washer on the housing face inside the stub bore, then two M8 nuts locked against each other. Tighten the first nut until the output just turns freely, then lock the second against it hard. The bolt end stops at about X 135, inside the stub bore, so it can't reach anything.

If the spacer prints long or short, sand it or shim it. The check is the running gap at the cheek, by feeler gauge, 0.8 to 0.9 mm all round.

## Bought parts for the fixture

As well as the arm hardware in `../../BUY_LIST.md`:

| Part | Qty | Use |
| --- | --- | --- |
| M8 × 55 hex head bolt, partly threaded | 1 | Head in the output pocket, through spacer and housing |
| M8 nut | 2 | Locked together in the stub bore, also the wrench flats for test 5 |
| M8 washer | 1 | On the housing face |
| Ø4 × 8 steel dowel pin | 3 | Output to fork, in place of printed pins |
| M3 hex nut | 6 | Housing pockets, one on each hoop sleeve |
| M4 hex nut | 6 | Output pockets |
| M4 × 16 button head | 6 | Fork to dummy output, in place of M4 × 14, so the screws pass through the nuts |
| M6 bolts and spacers to suit your bench or board | 4 | Through the housing legs. Holes are Ø6.5 |
| M5 × 40 bolt and nut | 2 | Across the lever socket. Drill the lever Ø5.5, 20 mm apart |
| 2020 extrusion, 600 mm | 1 | Lever, in the 20.4 mm socket |

## Checked

- Re-sliced with these fixes. All plates fit, no slicer warnings. Settings match the P2 project except the outer brim, and 40% infill on the housing. The outer brim shows on every plate except the fit-check carrier, whose brim the slicer dropped.
- Fork sector, dummy output, spacer, carrier, elbow ring and sleeves against the dummy housing, elbow 0, 30, 60, 90, 110 and 124: no interference, only the contact at the 0 degree stop. The sweep doesn't include screw heads or the M8 bolt.
- Finger to hoop screw head: 0.55 mm by the stack (head top X 146.55, finger tip X 146.0). Check it by hand at assembly, because print tolerance can use that up.
- Fork sector: the enclosed void, the 0.24 mm skin and the 0.9 mm crown wall are gone. The void and skin are also fixed in the live fork. The sector's tube section and crown are solid, so it isn't a test of the real forearm crown.
