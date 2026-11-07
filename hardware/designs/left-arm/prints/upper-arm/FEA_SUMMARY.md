# Left upper arm FEA summary, printed joint

2026-11-07. SolidWorks Simulation, PETG E 1900 MPa, Poisson 0.38, yield 45 MPa, layer factor 0.5, so layer allowable 22.5 MPa. Layers lie across X. Results stay on Joey-PC, not in git.

**Stage:** the robot stays stationary and supported, so falls and tip-over aren't load cases. This is the break-it-now pass. The one hard rule is that a printed part gives before an RS02 or RS03 is damaged. Targets: SF 2.0 and layer 1.5 on the arm, 1.5 and 1.3 on the carrier at 17 N·m.

## What changed

- **No steel rings or sleeves.** Joey's rule: print everything except fasteners and bearings. The six M3 screws clamp the carrier and hoop straight onto the RS02 shoulder. DIN 9021 washers sit under the heads at 0.35 N·m, about 580 N and 10 MPa. The hub M4s each get two DIN 125 washers at 0.6 N·m.
- **Hoop holes Ø3.3, hub holes Ø4.3.** The hoop ligament from hole to bore pad grows from 1.1 to 2.05 mm.
- **Carrier:** the ring's 2 mm is folded in, so the carrier face meets the hoop at X 151.7. There's a Ø9.4 seat for a DIN 9021 and a DIN 6796. The floor is at X 149.75 in CAD and prints at 149.7, leaving a 2.0 mm web. Head top is at X 146.65 or higher once the spring washer is loaded, so the finger clears by at least 0.65 mm. The pilot is gone; the screws locate the carrier in Ø3.2 holes.
- **Hot spots:** an r 1.5 fillet along both concave edges where the hoop neck meets the tube wall, X 152 to 165, and the fuse pad gap closed. The inboard root corner at X 151.7 has a 2.5 mm chamfer on both sides now; R4 found the +Z one missing. A seat lip was tried and removed, because its root became the new peak.
- **Washer capture, cover bosses at Y 268, slow-cure epoxy:** the capture stack and the epoxy are gone with the sleeves. The Y 268 bosses are 6.85 to 8.83 deep, with a 1 mm floor.

## Joint model

The hoop is PETG and the RS02 is a 10 mm steel rim on the seat. Six M3 rods are bonded to the rim and pass through the Ø3.3 holes with 0.15 mm clearance. A 2.8 mm PETG plate stands in for the carrier web. Preload is 580 N per screw, applied over a Ø9 patch on the web and on the rim. Contacts: PETG on PETG and PETG on steel, friction 0.3; rod in hole, 0.2. The arm is fixed at the six hub holes and the load goes on the rim. Mesh is 2 mm, with 0.8 on the hole walls, the seat and the corner.

| Case | Load at the elbow axis | Worst hoop SF, at the hole edge | Seat face near the RS02 shoulder edge | Ligament mid-thickness | Hoop-to-tube corner | Layer SF | Seat opens, mm | Slip, mm |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| LC1A | 300 N along Z, 17 N·m about X | 1.88 | 1.80 | 4.91 | 4.00 | 2.03 | 0.001 | 0.006 |
| LC1B | LC1A reversed: −300 N Z, +17 N·m X | 2.33 | 2.33 | 5.04 | 4.21 | 2.83 | 0.001 | 0.007 |
| LC1C | 300 N along X, −17 N·m about X | 2.01 | 2.01 | 4.47 | 2.95 | 2.45 | 0.009 | 0.003 |
| LC2 | 17 N·m about Y, the arm axis | 2.92 | 3.29 | 4.91 | 5.33 | 2.79 | 0.003 | 0.001 |
| LC3A | 120 N along X, 34 N·m about Z, sideways hand contact | **0.74** | **0.61** | 2.32 | 1.19 | **0.56** | 0.030 | 0.009 |

**LC1 and LC2: nothing is grossly under.** The worst is SF 1.80 on the seat face at r 39.2, right at the edge of the RS02 shoulder contact. That edge is where a stiff part ends on PETG, so it reads high on any mesh. The ligament is no longer a concern at 4.5 or more, the corner is at 2.95 or more, and every layer SF is 2.0 or more. The joint doesn't slip or open under these loads.

**LC3A fails.** A sideways push of 120 N at the hand pries the hoop on the RS02 shoulder. The seat edge and the port-slot corner yield locally at about 0.6, and the joint opens 0.03 mm. That's the printed part giving first, which keeps the rule. But it means **no sideways bench pushes over about 40 N at the hand** until rig test 7 measures the real limit. If the arm needs to take side loads later, that's the case for stepping up material.

## Carrier at 17 N·m

444 N on the stop band at r 38.25, 1 mm mesh, about 330,000 nodes. The screw holes are fixed rigidly, with no pilot. With 580 N preload per screw and friction 0.3, the clamp alone carries about 38 N·m at r 36.5. **That's a fresh-clamp upper bound.** After the PETG relaxes it is more like 12 to 30 N·m. Once the clamp slips, the two-hole case is the bound for the carrier, and bearing on the hoop ligament takes over. Screw bending into the RS02 isn't modelled, so watch for it in rig test 5.

| Case | SF | Layer SF | Where |
| --- | --- | --- | --- |
| Stop 0, all six holes | 2.14 | 1.68 | Hole wall in the web |
| Stop 124, all six holes | 1.84 | 1.35 | Stop wall root |
| Stop 0, only the nearest two holes | 1.50 | 1.74 | Hole wall in the web |
| Stop 124, only the nearest two holes | 1.61 | 1.18 | Stop wall root |

These are on the R4 carrier, with the 2.0 mm web under the spring washer.

All six holes pass. The two-hole case is the bound once the clamp has slipped. It meets SF 1.5 but misses the 1.3 layer target by 0.12 at the 124 stop. The stop is meant to be the weak link, and rig test 5 measures it.

## Failure order

Without steel, the screws carry about a third of the earlier preload, so the RS02 threads see less. The joint slips or the PETG crushes long before an M3 thread is at risk. Under overload, the first thing to give is either the printed stop (17 N·m rated, 25.5 to 34 N·m to break in the rig) or, for side loads, the hoop seat edge. Both are printed. The RS02 output bearing still carries every side moment directly, and its rating is unknown, so the side-load limit above stays until the rig measures it.

## Fit

| Check | Result |
| --- | --- |
| Hoop button heads to fork finger | 0.6 mm or more at the upper three holes, 6.8 mm or more at the lower three |
| Cover to arm overlap | About 1.0 mm³ at every angle, a sliver at the seating rim. The earlier 0.04 mm³ figure was wrong |
| Fork cheek running gap | 0.9 mm on the arm, 0.8 mm in the rig. See `../../MOVING_INTERFACES.md` |
| Bed contact on the upper arm | About 287 mm² in the first layer |

## Open

- **Clamp creep:** PETG under the washers relaxes. Assembly retorques at 1 hour and 1 day, and rig test 8 measures it.
- **Side-load limit:** LC3A above. Rig test 7 gives the real number.
- **RS02 ratings:** RobStride gives no bearing or thread rating, so failure order against the actuator stays a comparison against the rig's printed failure loads.
