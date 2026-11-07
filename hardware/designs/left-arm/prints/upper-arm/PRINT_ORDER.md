# Left upper arm print order

2026-11-07. **Released 2026-11-07 after the R4 recheck**, see `GATE_REVIEW.md`. Everything prints except fasteners and bearings.

**Decision (Joey):** the upper arm prints now, and the elbow rig test runs before the forearm is fitted, not before this print.

Settings are the test pass 1 process: P2S, 0.4 mm High Flow, Generic PETG, or Bambu PLA Basic where the table says PLA, Cool Plate Super Tack with no glue, bed 70 °C, 0.20 mm, 6 walls, 100% infill, tree supports (Tree Hybrid), outer 5 mm brim. See `../test-pass-1/SETTINGS_CHECKLIST.md`. Open each file as a project, don't rotate anything, and check the settings before printing. Each plate matches the P2 L04B project except `brim_type`, which is `outer_only` in place of `auto_brim`. No slicer warnings on any plate.

| Order | File | Material | Print time | Filament | Down on the plate | Why |
| --- | --- | --- | --- | --- | --- | --- |
| 1 | `seat_ring_only.3mf` | PETG | 21 m | 9.2 g | Same way up as the carrier | Only the Ø52.2 seat, Ø50 shoulder and Ø40 stub. The 6808 has never been fitted to a print, so check that one fit first. 3 walls, 15% infill |
| 2 | `carrier.3mf` | PETG | 1 h 25 m | 36.4 g | Inboard face | Same way up as the strength carrier in test pass 1 |
| 3 | `cover.3mf` | Bambu PLA Basic | 24 m | 3.8 g | Standing on its end | Thin curved shell |
| 4 | `upper_arm.3mf` | PETG | 8 h 7 m | 229.8 g | Flat on the hub, X 131.99 | Layers across X, as in the FEA. The RS02 seat faces up, clean. **50% infill, with 100% over the hoop body and seat, the hoop-to-tube corners and the hub block** |
| 5 | `p2.3mf` | PETG | 5 h 56 m | 187.3 g | As P2 L04B, same 45 degree plate rotation | Only if you want a new P2, see below |

**Neck cleanup, 2026-11-07, second pass:** the bed-side spikes in the first cleanup were the coarse STL facets of the tube cutting through the r1.5 root fillet, plus chamfer faces running past the clip. The arm is now exported at 0.01 mm / 5 degrees, the clip follows the tube cone out to Y 257.45 (NECK_CLIP2 to 7), the chamfer ends stop at the tube circle, and the relief pockets open cleanly into the bore (RELIEF_OPEN, PORT_MERGE). The build runs `hardware/tools/mesh_check.py` before writing the 3MF; see `upper_arm_meshcheck.txt`, `renders/upper_arm_sheet.png` and `renders/upper_arm_neck_bedside.png`.

Total for plates 2 to 5: about 15 h 55 m and 457 g, including support and brim. Without P2, 9 h 58 m and 270 g.

The fit ring has a thin wall, so it reads slightly looser than the carrier. Pull any low stubs of support out of the carrier seat before pressing the bearing.

## P2

P2 L05 is P2 L04B with 2.5 mm off the inboard tip. It clears the M4 heads on the r 49 collar ring, so the inward roll limit can go from 2 to 5 degrees. **You don't have to reprint it.** You can file 2.5 mm off the tip of the P2 you have, flat, over Z 8 to 28, for the same clearance. A reprint uses the same process and orientation as L04B.

## Where support lands

Each is from the sliced G-code.

| Part | On support | Keep clear |
| --- | --- | --- |
| Carrier | Recess roof, retainer pockets, under the head pockets | **A support blocker keeps support out of the bearing seat**, apart from a few low stubs near the bed. The ledge prints unsupported, so check it is flat. The outboard face prints up, clean |
| Upper arm | **Hoop inboard face at X 151.79**, tube inside, port slot edges, **all six Ø4.3 hub holes** and the notch at the bottom of the bore | RS02 seat at X 165.19, hub face at Y 319.86 |
| Cover | Cable entry and mounting holes. **About 27% of its extrusion is support** | Seating rim, mostly |

The hoop inboard face can't avoid support in this orientation. Flipping the arm moves support onto the RS02 seat instead, and adds 13 g and 23 minutes.

The bed contact on the upper arm was 15 mm². The new flat on the hub gives about 287 mm² in the first layer, so no raft. The rest of the arm stands on support, so watch the first layers.

## After printing

- **Hoop face:** sand it flat, see `ASSEMBLY_ORDER.md`, step 2.
- **Carrier:** clear the recess and pockets without marking the seat. Check the ledge is flat.
- **Upper arm:** take the tube support out through the cable window and port slot in small pieces. Clear each hub hole with a Ø4.3 drill by hand.

Parts list: `PARTS_LIST.md`. Assembly: `ASSEMBLY_ORDER.md`. Strength: `FEA_SUMMARY.md`. Hashes: `SHA256SUMS.txt`.
