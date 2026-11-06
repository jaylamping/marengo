# Pack cradle, rev A (as modelled)

SolidWorks model of [CRADLE_SPEC.md](CRADLE_SPEC.md), in torso coordinates, 2026-11-06. The web inner face is at **|X| 48.0**, not the spec 47.5, so the pack slides with 0.5 mm per side. **Not gate reviewed, no 3MF yet.**
Native parts are in `J:\code\marengo\cad\torso\` (`marengo_torso_cradle_*`, named per `cad/manifests/cad-conventions.json`). `build_cradle.py`
rebuilds them, `check_overlaps.py` and `mass_estimate.py` give the numbers below, `render_cradle.py` makes `CRADLE_ASSEMBLED.png`.

## Parts

| Part | Qty | Volume, mm3 | Solid PETG, g | 4 walls / 15 % gyroid, g (estimate) |
| --- | --- | --- | --- | --- |
| `marengo_torso_cradle_side_right` (-X) | 1 | 55,370 | 70.3 | 61 |
| `marengo_torso_cradle_side_left` (+X) | 1 | 55,370 | 70.3 | 61 |
| `marengo_torso_cradle_top_strap` | 1 | 7,017 | 8.9 | 8.9 |
| Set | | | 149.5 | 132 |

Solid mass is SolidWorks volume × 1.27 g/cm3. The gyroid figure is a voxel estimate with 1.68 mm walls, so the 3.5 mm web prints almost solid.
Windows open 34 % of the web. The left side is the same script with X mirrored, not a linked mirror feature.

## Fasteners

- 8 × M5 × 10 button head + drop-in T-nut: bottom feet at Z ±12, top feet at Z −30 and +20.
- 2 × M4 × 16 thumbscrew: head seats on the boss face at |X| 59, tip at |X| 43: 4.5 mm past the centred pack side, 4 to 5 mm across the 0.5 mm float (limit 6). The boss is 7.5 long.
- 4 × M3 heat-set insert, 4.2 pilot, 5.5 deep, at |X| 51.75, Y 200 and 330. The carrier needs matching holes.

## Changes from the spec

1. **Bottom foot (item 2) did not fit.** Corner brackets br1, br6, br11 and br16 lie on the bottom side rail top face over Z ±22 to ±50, with screw heads at Z ±33.5.
   The spec foot overlapped them by 20 × 4 × 22 mm at the rear and 20 × 4 × 15.5 mm at the front, and its slots at Z −30 and +20 landed on or beside bracket screws.
   Now a Z −44 to +37.5 strip at |X| 51.5 to 55, plus a tongue over the rail at Z ±21.5 with the slots moved to Z ±12. The shelf rail has no brackets, so the top foot is as specified.
2. **Front lip (item 5) starts at Y 179, not 183.** At Y 183 it met the ledge on a bare edge, which is not a valid solid.
3. **Carrier pads (item 7) are blocks.** An M3 insert does not fit a 3.5 mm web, so each pad is 7.5 × 10 × 8 at |X| 48 to 55.5.
4. **Top strap could not hook over the webs.** The webs run up to the top feet, so nothing passes over them, and a 102 mm strap cannot enter closed slots across a 95 mm gap.
   Each web now has a slot, Y 345.5 to 348.9, open at the rear edge up to Z 5.2. The strap slides in from the rear after the pack and clicks into a Ø4.4 detent pocket at Y 342.5, Z −5.
   The strap gained outer lips to |X| 54.2 and is 103.4 long across the webs. No extra fasteners. This costs web material: from Z −50 to 5 only a 4.7 mm strip joins the web to the top foot.
5. **Added:** 3.5 × 3.5 root chamfers between web and feet (bottom full length, top forward of the slot).
6. **Pack clearance.** Webs moved out to |X| 48 to 51.5, with the boss, pads, strap slots and strap lips moving with them. The M5 slots stay at |X| 65 on the T-slot. The ledge and lip keep their inner edges and now reach the web at |X| 48. The strap sits Y 345.5 to 348.5.

## Overlaps and clearance

`check_overlaps.py` tests every part in `generator/parts.json`, on boxes and again on the exported meshes at 0.4 mm voxels.

| Against | Result |
| --- | --- |
| Extrusions, corner brackets, their fasteners, brake plate and resistor, `harness_side_*`, gussets, shelf plate | No overlap. Touching at the mounting faces, 0.5 mm from the foot tongue to the bracket legs, 0.98 mm to the shelf plate |
| `packrail_*` | Overlaps throughout: the cradle replaces these, and `parts.json` still lists them |
| `pdb_sheet` | **Real.** The lip takes 5.5 × 150 × 0.5 mm per side: the sheet must narrow to \|X\| ≤ 42 |
| `harness_pack` | **Real.** The strap takes Y 345.5 to 348.5 of this reserve, leaving Y 348.5 to 353.6 (5.1 mm) for the 10 AWG pair and sense harness, 6.1 mm to the shelf plate |
| `pdb_carrier` | Touches the Z 37.5 face, as intended |
| Pack | 0.5 mm per side at the webs, 0.5 mm under the strap, 0 mm at the ledge and lip, which bear it |
| Strap to side | No overlap: 0.2 mm per side at the web, 0.4 mm in slot height |

Both thumbscrews pull the pack toward their own web, so with the pack centred, tightening closes the 0.5 mm gaps by bowing the webs inward. That needs a check at assembly: it may want a spacer or a limit on tightening.

Not checked: strength (no FEA), and the thumbscrew head against the shell (assumed Ø18 × 6.5 for the frame check).
Print suggestion, untested: stand each side on its rear edge so the C section lies in the layer plane.
