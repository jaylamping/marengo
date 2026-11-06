# Test pass 1 print order

These are test pieces, not load-bearing arm parts. Don't fit them to the robot, apart from the PF03 spacer. Settings: `SETTINGS_CHECKLIST.md`. Tests: `../../testing/ELBOW_DUMMY_OUTPUT_TEST.md`.

| Order | File | Print time | Filament | Up on the plate | Why |
| --- | --- | --- | --- | --- | --- |
| 1 | `PF03_roll_spacer.3mf` | 1 h 0 m | 23.6 g | Axis vertical | Reprint at 100% infill. The last one weighed 11 g |
| 2 | `layer_coupons.3mf` | 2 h 39 m | 69.1 g | 5 bars standing, 5 flat | Real layer strength, test 1 |
| 3 | `elbow_carrier.3mf` | 1 h 9 m | 29.5 g | Outboard face on the bed | Fit with the real 6808 and rings, tests 2 and 3 |
| 4 | `dummy_output.3mf` | 28 m | 6.7 g | Pins up | Carries the outboard cheek |
| 5 | `fork_sector.3mf` | 7 h 33 m | 265.1 g | Outboard cheek up | Stop and failure tests |
| 6 | `dummy_housing.3mf` | 9 h 56 m | 391.6 g | Carrier face on the bed | Stands in for the RS02 and upper arm |

Total: about 23 h 45 m and 786 g.

**Gate 2 review, 2026-11-06 (`GATE2_REVIEW.md`):** plate 1 GO. Plate 2 GO with an outer 5 mm brim set by hand. Plate 3 GO as a fit-check piece only. **Plates 4, 5 and 6 are on hold** until the fixes listed there are made and the files re-sliced.

**Stop after step 2 or 3 if something is off.** Weak coupons change every layer number, and a bad 6808 fit means the CAD changes before the bigger prints. Steps 5 and 6 take most of the plastic, so don't start them until steps 2 and 3 look right.

Tests 5 and 6 break parts. If the first set goes well, print a second carrier and fork sector.

## After printing

- **Coupons:** remove the brim gently, and don't flex the standing bars.
- **Carrier:** clear all support from the recess, counterbores and bearing seat. Check the bridge over the recess.
- **Dummy output:** has support inside the six M4 nut pockets on the bottom. Pick it out, then press the nuts in.
- **Fork sector:** has a lot of support. Take it out in small pieces, and don't pry against the stub, finger or cheek faces.
- **Dummy housing:** press M3 nuts into the six hex pockets from the back.

## Bought parts for the fixture

As well as the arm hardware in `../../BUY_LIST.md`:

| Part | Qty | Use |
| --- | --- | --- |
| M8 × 45 bolt, nut and washer | 1 | Pin through the housing, head in the stub bore |
| M3 hex nut | 6 | Housing pockets, one on each hoop sleeve |
| M4 hex nut | 6 | Output pockets |
| M4 × 16 button head | 6 | Fork to dummy output, in place of M4 × 14, so the screws pass through the nuts |
| M6 bolts and spacers to suit your bench or board | 4 | Through the housing legs. Holes are Ø6.5 |
| M5 × 40 bolt and nut | 2 | Across the lever socket. Drill the lever Ø5.5, 20 mm apart |
| 2020 extrusion, 600 mm | 1 | Lever, in the 20.4 mm socket |

## Checked

- Fork sector, dummy output, carrier, elbow ring and sleeves against the dummy housing, elbow 0 to 124: no interference, only the contact at the 0 degree stop.
- All six slices: plate fits, no warnings, settings match the P2 project.
- **Not done:** the gate 2 before-print review of the carrier and fixture. The fork sector tests the stop, and its tube is cut off, so the open forearm twist result doesn't apply to it. The elbow ring must be cut before step 3 can use it.
