# Test pass 1 print order

These are test pieces, not load-bearing arm parts. Don't fit them to the robot, apart from the PF03 spacer. Settings: `SETTINGS_CHECKLIST.md`. Tests: `../../testing/ELBOW_DUMMY_OUTPUT_TEST.md`.

| Order | File | Print time | Filament | Down on the plate | Why |
| --- | --- | --- | --- | --- | --- |
| 1 | `PF03_roll_spacer.3mf` | 1 h 0 m | 23.7 g | Axis vertical | Reprint at 100% infill. The last one weighed 11 g |
| 2 | `layer_coupons.3mf` | 2 h 43 m | 70.3 g | 5 bars standing, 5 flat | Real layer strength, test 1 |
| 3 | `elbow_carrier.3mf` | 1 h 15 m | 32.8 g | Outboard face | Fit check only, with the real 6808, tests 2 and 3 |
| 4 | `elbow_carrier_strength.3mf` | 1 h 26 m | 37.6 g | Inboard face | The carrier for the stop tests |
| 5 | `dummy_output.3mf` | 46 m | 14.2 g | Cheek face, spacer beside it | Output disc and the M8 spacer |
| 6 | `dummy_housing.3mf` | 5 h 35 m | 192.7 g | Carrier face | Stands in for the RS02 and upper arm. 25% infill |
| 7 | `fork_sector.3mf` | 8 h 49 m | 343.9 g | Outboard cheek | Stop and failure tests |

Total: about 21 h 37 m and 715 g, including support and brim.

**2026-11-07:** the housing and both carriers changed to the all-printed joint, so nothing in the rig is steel except fasteners and the bearing. The dummy output and fork sector geometry didn't change; two DIN 125 M4 washers replace the 2 mm hub ring in the fork sector seat, and the M4 × 16 still passes through the nuts.

**Gate 2 review, 2026-11-06 (`GATE2_REVIEW.md`):** the fixes from the review and recheck R2 are made in these files. Plates 5 and 7 changed again after R2, so have those two looked at once more before printing.

**Stop after step 2 or 3 if something is off.** Weak coupons change every layer number, and a bad 6808 fit means the CAD changes before the bigger prints. Steps 6 and 7 take most of the plastic, so don't start them until steps 2 and 3 look right.

Tests 5 and 6 break parts. If the first set goes well, print a second carrier and fork sector.

## After printing

- **Coupons:** remove the brim gently, and don't flex the standing bars.
- **Carrier, both prints:** clear all support from the recess, counterbores and bearing seat. Check the bridge over the recess. Scrape the lip flat.
- **Strength carrier:** this one has the most support to clear. Clean the Ø49.5 to 52 seat ledge, the retainer pockets and the stop walls, then check the ledge is flat.
- **Dummy output:** clear the dowel holes with a Ø4 drill by hand, then press in the dowels. The slicer fills the six M4 nut slots with support. Clear each slot from the rim and test-fit a nut before assembly; file the slot if it is tight (7.1 × 3.3 mm for a 7.0 × 3.2 nut). Slide the six M4 nuts in from the rim, one in each slot, and line them up with a screw. The M8 head drops into the pocket in the back.
- **Fork sector:** has a lot of support under the inboard cheek, the finger and the stub. Take it out in small pieces, and don't pry against the stub, finger or cheek faces. Clear the Ø4 dowel holes by hand and drill them to about 4.5 mm deep (there is 1.5 mm of material below), so the 8 mm dowels can't hold the faces apart. Sand the Ø32 seat floor and the inner face of the stub cheek flat, because both print on support, and do it before the 0.8 mm feeler check.
- **Dummy housing:** press M3 nuts into the six hex pockets from the back. Screw holes are Ø3.3, no sleeves. The carrier clamps straight on the housing face, as it does on the arm hoop. Retorque the M6 leg bolts before each test.

## M8 stack

This sets the fork at its design position, with the 0.8 to 0.9 mm running gap at the carrier and the finger in full contact.

From the output outward: M8 × 55 hex head bolt, class 10.9, with its head in the output's hex pocket. Then the printed spacer (4.0 mm, measure it) between the output back and the housing core, then the housing bore, then a washer on the housing face inside the stub bore, then two M8 nuts.

1. Put thread-locker (medium strength, such as Loctite 243) on the bolt under both nuts.
2. Run the first nut on until the output just turns freely.
3. Hold it with a thin spanner and lock the second nut against it to 15 N·m, not more. The jam preload and the test torque add up in the bolt, and the thread-locker does the holding. Finish with their flats lined up, so a socket takes both nuts at once.
4. Let the thread-locker cure for a day, then draw a paint line across both nuts and the bolt end.

The bolt end stops at about X 129, 2.7 mm past the inboard cheek face, with nothing near it. The thread runs to about X 151, and the first nut sits at X 143 to 150, so it has about 1 mm of thread to spare. Check that by hand before the bolt goes in. If the spacer prints long or short, sand it or shim it. The check is the running gap at the cheek, by feeler gauge, 0.8 to 0.9 mm all round.

## Bought parts for the fixture

As well as the arm hardware in `../../BUY_LIST.md`:

| Part | Qty | Use |
| --- | --- | --- |
| M8 × 55 hex head bolt, class 10.9, **fully threaded (ISO 4017)** so the first nut is on full thread | 1 | Head in the output pocket, through spacer and housing. Class 8.8 is too close to yield in torsion at 17 N·m |
| M8 nut | 2 | Locked together in the stub bore, also the wrench flats for test 5 |
| M8 washer | 1 | On the housing face |
| Medium thread-locker | 1 | M8 nuts |
| 13 mm deep socket | 1 | Test 5, through the stub bore, on both nuts |
| Ø4 × 8 steel dowel pin | 3 | Output to fork, in place of printed pins |
| M3 hex nut | 6 | Housing pockets, one on each hoop screw |
| M4 hex nut | 6 | Output pockets |
| M4 × 16 button head | 6 | Fork to dummy output, in place of M4 × 14, so the screws pass through the nuts |
| M6 bolts and spacers to suit your bench or board | 4 | Through the housing legs. Holes are Ø6.5 |
| M5 × 40 bolt and nut | 2 | Across the lever socket. Drill the lever Ø5.5, 20 mm apart |
| 2020 extrusion, 600 mm | 1 | Lever, in the 20.4 mm socket |

## Checked

- Re-sliced with these fixes. All plates fit, no slicer warnings. Settings match the P2 project except the outer brim, Cool Plate Super Tack, and 25% infill on the housing. The outer brim shows on every plate except the fit-check carrier, whose brim the slicer dropped.
- Fork sector, dummy output, spacer and carrier against the dummy housing, elbow 0, 30, 60, 90, 110 and 124: no interference, only the contact at the 0 degree stop. The sweep doesn't include screw heads or the M8 bolt.
- Finger to hoop screw head: 0.55 mm by the stack (head top X 146.55, finger tip X 146.0). Check it by hand at assembly, because print tolerance can use that up.
- After recheck R2: no slit at Y 141 to 142 and no slots at the cheek roots, by rays through the mesh. What is left at the cheek roots is under 0.5 mm wide on the outer edge, and it prints closed. One solid body. The sweep above was rerun on the new output and spacer with the same result.
- Fork sector: the enclosed void, the 0.24 mm skin and the 0.9 mm crown wall are gone. The void and skin are also fixed in the live fork. The sector's tube section and crown are solid, so it isn't a test of the real forearm crown.
