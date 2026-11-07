# Left upper arm FEA and fit summary

2026-11-07. SolidWorks Simulation, E 1900 MPa, Poisson 0.38, yield 45 MPa, layer factor 0.5, on the final parts. Layer SF is across the print layers, normal X, for all parts here. Simulation results stay on Joey-PC, not in git. "Clear" means away from fixtures and load faces.

## Carrier at 17 N·m, with the ribbon slot

444 N on the stop band at r 38.25, fixed at the six sleeve holes. Target SF 1.5, layer 1.3.

| Stop | Mesh | Nodes | SF | Layer SF | Clear SF | Clear layer SF |
| --- | --- | --- | --- | --- | --- | --- |
| 0 | 1.5 mm | 108,975 | 2.38 | 2.83 | 2.39 | 6.13 |
| 0 | **1.0 mm** | 307,835 | **2.10** | **2.78** | 2.10 | 5.17 |
| 124 | 1.5 mm | 108,870 | 1.89 | 2.19 | 2.25 | 4.27 |
| 124 | **1.0 mm** | 306,820 | **1.89** | **2.08** | 2.04 | 2.56 |

**Passes.** Worst SF is unchanged by the finer mesh at the 124 stop, and drops 12% at the 0 stop. The slot itself sits at SF 2.8. The steel ring is not modelled.

## Ribbon slot

- **Location:** theta 180, at the top under the RS02 port slot, 19 wide, cut 1.5 mm into the carrier spigot, X 149.7 to 151.2. It stays 24 degrees or more from the hoop counterbores.
- **Route:** tube cavity, port slot (23 wide, ribbon 17.8), through the new notch in the steel ring, along the carrier slot, across the stub end, into the Ø30 bore.
- **Theta 330 not used.** It is on the far side of the elbow from the tube, so the ribbon would need about 100 mm of exposed run around the carrier.
- **The tube floor exit is not used.** It is 6 × 7.5 mm, too small for a 17.8 mm ribbon. The port slot replaces it.

## Upper arm

2 mm mesh, 487,499 nodes, on the arm with the new fuse pockets. Target SF 2.0, layer 1.5.

| Case | SF | Layer SF | Clear SF | Clear layer SF |
| --- | --- | --- | --- | --- |
| LC1A | 2.27 | 1.61 | 2.27 | 2.66 |
| LC1B | 2.27 | 1.87 | 2.27 | 2.26 |
| **LC1C** | **1.79** | **1.16** | 2.44 | 1.72 |
| LC2 | 2.41 | 1.71 | 3.73 | 3.46 |

**LC1C misses both targets at the M3 holes.** Its peaks are at the sleeve hole edges in the hoop, X 151.7 and X 165.2, the faces the load is applied to in this model. Away from those faces it passes. The steel sleeves and ring are not modelled and should carry load there, but that isn't shown, and the number got worse from the 3 mm mesh (2.15, 1.66). **This is for the review to judge.**

## Fuse pockets

Two pockets, 12.7 wide, Y 260 to 290, on the wall opposite the window. Divider 1.2 mm in place of 0.6, outer walls 1.7 mm. Adds 0.4 cm³.

## Fit

| Check | Result |
| --- | --- |
| Elbow 0 to 124 by 5 degrees, with yaw 0, and ±60 at 0, 90 and 124 | Clear. Stop contact at 0 as designed. Cover to arm overlap of 0.04 mm³ at every angle, unchanged from rev C |
| Yaw ±90 by 15 degrees, arm, carrier and cover against P2 and spacer | 32 mm minimum |
| Collar M4 heads to P2 | 7.5 mm at roll 0, 5.6 at 2 degrees, 2.8 at 5 degrees. Was 1.0 at 5 degrees |

The sweep has no ribbon, screw heads or fuse holders in it.

## Open

- **Fork stub end:** the ribbon crosses it in a 1.34 mm gap to the RS02 rear, X 150.4 to 151.74, for a 1 mm ribbon. That's tight next to a turning part. Shorten the stub by about 0.5 mm when the fork is next changed, after checking the bearing seat. This doesn't affect these prints.
- **Ribbon into the bore:** it needs a 45 degree fold at the top of the bore to turn its width along X. Not modelled, try it on a paper strip first.
- **Test pass 1 carriers have no slot.** The stop tests still apply, with this FEA covering the slot.
- **Fork strength** is still short of target, see the forearm notes. It doesn't affect these prints.
- **Cover screw length** is still open.
- **Elbow ring:** the notch leaves 4 mm of steel at theta 180.
