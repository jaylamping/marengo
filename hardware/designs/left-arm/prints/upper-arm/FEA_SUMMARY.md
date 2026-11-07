# Left upper arm FEA and fit summary

2026-11-07. SolidWorks Simulation, PETG E 1900 MPa, Poisson 0.38, yield 45 MPa, layer factor 0.5, so layer allowable 22.5 MPa. Steel E 200 GPa. Results stay on Joey-PC, not in git. Layer SF is normal stress across the print layers, normal X, for all parts here.

## What changed after the gate review

- **Ribbon:** Joey chose the rolling loop on the back of the elbow under a printed guard. B1 is dropped: no slot in the carrier, no notch in the ring, and the tube floor exit is closed.
- **My error in the last package:** the carrier slot was cut on the wrong side of its plane, X 148.1 to 149.7, so its FEA was for the wrong part. That slot is gone. Every cut in this package was checked with rays through the exported mesh.
- **Steel-only clamp:** hoop 13.40 thick, sleeves 13.45. Hub 21.80, sleeves 21.86. The hoop faces land on the 0.2 mm layer grid: bed at X 131.99, faces at X 151.79 and 165.19.
- **Fit clearance:** Ø5.2 hoop holes, Ø6.2 hub holes, Ø52.2 bearing seat. Pads in the Ø66 bore at each hole bring it to Ø65.6 locally. The RS02 body is Ø65. The wall from hole to bore is 1.1 mm.
- **Carrier:** a pilot in the Ø66 ring, r 32.9, 0.1 mm clearance. Hole walls Ø6.05 and 1.5 tall with three steel washers, in place of Ø6.4 and 1.0 tall.
- **Fuses:** rows across the tube for 35 mm holders, wedge and void under the pad filled.
- **Root corner:** 2.5 mm chamfer at X 151.7, Y 254.9, and the floor exit filled.

## Load cases

Load at the elbow axis, arm fixed at the six top hub holes.

| Case | Force | Moment | Source |
| --- | --- | --- | --- |
| LC1A | 300 N along Z | 17 N·m about X | 60 N·m at the shoulder over 215 mm is 279 N, plus 2× gravity on 1.07 kg. 17 N·m is the RS02 peak |
| LC1B | LC1A reversed | | |
| LC1C | 300 N along X, lateral | 17 N·m about X | Same force in the shoulder roll direction |
| LC2 | None | 17 N·m about Y, the arm axis | Yaw RS02 peak |

## Upper arm joint model

The last package's SF 1.79, layer 1.16 for LC1C came from loading the bare hole walls. This model has steel sleeves, the elbow ring and an RS02 rim as bodies, bonded to each other, and the PETG hoop touching them only through no-penetration contact on the hole walls and both hoop faces. Sleeve bending at the hole edges is included.

**Preload is not a separate load.** At 1.0 N·m the M3 screws carry about 1670 N, against about 130 N of shear per screw in the worst case, so the steel stack doesn't slip and acts as one rigid part. This assumes that, it doesn't model it. PETG nodes on the steel contact surfaces can't be told apart from steel nodes, so they're left out. The first PETG row is about 0.02 mm off the wall.

Target SF 2.0, layer 1.5. 2 mm mesh with 0.8 mm on the hole walls.

| Case | SF, worst PETG node | SF, 0.5 mm off the hole | SF, 1.0 mm off the hole | Layer SF |
| --- | --- | --- | --- | --- |
| LC1A | 1.54 | 1.91 | 1.91 | 2.71 |
| LC1B | 1.34 | 1.99 | 2.32 | 3.38 |
| LC1C | 1.65 | 1.96 | 1.96 | 1.73 |
| LC2 | 2.81 | 3.64 | 4.16 | 2.05 |

**Layer SF passes on all four. Von Mises at 1.0 mm off the hole passes LC1B and LC2 and misses by a little on two cases**, both away from the hole wall: LC1A 1.91 on the seat face at r 39.5, where the edge of the RS02 shoulder lands, and LC1C 1.96 at the hoop-to-tube corner, X 152.8, Y 254.9, Z −22. Neither is the 1.0 mm ligament. I've left these for the review rather than add more material.

**Mesh check, LC1C before the root chamfer, 0.8 and 0.5 mm on the hole walls:** worst node 24.9 to 28.1 MPa, rising, 0.5 mm off 23.2 to 26.1, rising, 1.0 mm off 22.10 to 22.19, settled. So the worst node is an edge singularity where the rim edge lands on the hoop face, and the stress 1 mm away has converged. The 0.5 mm figure has not fully converged, so judge on the 1.0 mm column and treat the first millimetre as local yield at the contact edge.

## Carrier at 17 N·m

444 N on the stop band at r 38.25, 1 mm mesh, about 314,000 nodes. Pilot held radially. Target SF 1.5, layer 1.3.

| Case | SF | Layer SF | Clear SF | Clear layer SF |
| --- | --- | --- | --- | --- |
| Stop 0, all six holes | 2.35 | 2.27 | 2.6 | 3.78 |
| Stop 124, all six holes | 2.63 | 2.26 | 3.34 | 4.18 |
| Stop 0, only the nearest two holes | 2.34 | 2.26 | 2.64 | 3.75 |
| Stop 124, only the nearest two holes | 2.63 | 2.25 | 3.35 | 4.12 |

**Passes.** The numbers barely change with fewer holes, because the peak is at the stop wall. Bearing on the hole walls by hand: 17 N·m is 78 N per hole if all six share, on 9 mm², 8.7 MPa. With two holes it's 26 MPa, which is why the holes get opened to fit the washers. This model fixes the hole walls rigidly, without clearance.

## Fit

| Check | Result |
| --- | --- |
| Elbow 0 to 124 by 5 degrees, yaw ±60 at 0, 90 and 124 | Clear. Stop contact at 0 as designed. Cover to arm overlap of 0.04 mm³ at every angle, unchanged from rev C |
| Hoop button heads to fork finger, all elbow angles | 0.55 mm at the three upper holes, 6.8 mm or more at the lower three |
| Yaw ±90, arm against P2 and spacer | 32 mm minimum |
| Collar M4 heads to P2 | 7.5 mm at roll 0, 5.6 at 2 degrees, 2.8 at 5 degrees |

## Open

- **Failure order:** printed parts failing before the actuators is still not shown. RobStride gives no bearing or thread rating. The rig test gives the printed side only.
- **Hole-edge yield:** the first millimetre at the hoop seat face may yield locally at peak load. It's on the steel-only stack, so it shouldn't lose preload, but the rig test should watch for it.
- **Sleeve fit:** the retaining compound is assumed to give full bearing round each hole. Without it, a 0.1 mm gap puts the load on fewer holes.
- **Test pass 1 carriers** have the old holes and no pilot, so they're fit checks for this carrier, not strength proof of it.
- **Fork strength** is still short of target, see the forearm notes. It doesn't affect these prints.
