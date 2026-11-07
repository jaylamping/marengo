# Pack cradle, rev A (as modelled)

SolidWorks model of [CRADLE_SPEC.md](CRADLE_SPEC.md), in torso coordinates, 2026-11-06. The web inner face is at **|X| 48.0**, not the spec 47.5, so the pack slides with 0.5 mm per side. **Not gate reviewed, no 3MF yet.**
Native parts are in `J:\code\marengo\cad\torso\` (`marengo_torso_cradle_*`, named per `cad/manifests/cad-conventions.json`). `build_cradle.py`
rebuilds them, `check_overlaps.py`, `check_hook_fit.py` and `mass_estimate.py` give the numbers below, `render_cradle.py` makes `CRADLE_ASSEMBLED.png`.

## Parts

| Part | Qty | Volume, mm3 | Solid PETG, g | 4 walls / 15 % gyroid, g (estimate) |
| --- | --- | --- | --- | --- |
| `marengo_torso_cradle_side_right` (-X) | 1 | 55,370 | 70.3 | 61 |
| `marengo_torso_cradle_side_left` (+X) | 1 | 55,370 | 70.3 | 61 |
| `marengo_torso_cradle_top_strap` (two snap hooks) | 1 set | 4,653 | 5.9 | 5.9 |
| Set | | | 146.5 | 129 |

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
   Each web now has a slot, Y 345.5 to 348.9, open at the rear edge up to Z 5.2. Each hook fits from the side and its pin seats in a Ø4.4 detent pocket at Y 342.5, Z −5 (see Hook retention).
   The strap gained outer lips to |X| 54.2 and is 103.4 long across the webs. No extra fasteners. This costs web material: from Z −50 to 5 only a 4.7 mm strip joins the web to the top foot.
5. **Added:** 3.5 × 3.5 root chamfers between web and feet (bottom full length, top forward of the slot).
6. **Pack clearance.** Webs moved out to |X| 48 to 51.5, with the boss, pads, strap slots and strap lips moving with them. The M5 slots stay at |X| 65 on the T-slot. The ledge and lip keep their inner edges and now reach the web at |X| 48. The strap sits Y 345.5 to 348.5.

## Overlaps and clearance

`check_overlaps.py` tests every part in `generator/parts.json`, on boxes and again on the exported meshes at 0.4 mm voxels.

| Against | Result |
| --- | --- |
| Extrusions, corner brackets, their fasteners, brake plate and resistor, `harness_side_*`, gussets, shelf plate | No overlap. Touching at the mounting faces, 0.5 mm from the foot tongue to the bracket legs, 0.98 mm to the shelf plate |
| `packrail_*` | Overlaps throughout: the cradle replaces these, and `parts.json` still lists them |
| `pdb_sheet` | Resolved: the sheet is now \|X\| ≤ 42 in `parts.json`; it meets the lip face at \|X\| 42 with 0 mm (cut it 1 mm narrower, \|X\| ≤ 41, at assembly) |
| `harness_pack` | Clear. The reserve is now X −5 to 25, in the strap gap, so the leads have the full Y 345 to 353.6 (8.6 mm) |
| `pdb_carrier` | Touches the Z 37.5 face, as intended |
| Pack | 0.5 mm per side at the webs, 0.5 mm under the strap, 0 mm at the ledge and lip, which bear it |
| Hook to side | No overlap: 0.2 mm at the web, 0.4 mm in slot height |

Both thumbscrews pull the pack toward their own web, so with the pack centred, tightening closes the 0.5 mm gaps by bowing the webs inward. That needs a check at assembly: it may want a spacer or a limit on tightening.

Not checked: strength (no FEA), and the thumbscrew head against the shell (assumed Ø18 × 6.5 for the frame check).
Print suggestion, untested: stand each side on its rear edge so the C section lies in the layer plane.

## Lead gap (2026-11-07)

The pack power leads (10 AWG silicone, 5.2 to 5.8 mm OD) cross the pack top along Z and did not fit the 5.1 mm left above the strap.
The strap now has a **30 mm gap at X −5 to +25**, through its full height and depth (`Lead_gap` in `build_cradle.py`), so the leads get Y 345 to 353.6 (8.6 mm) there.
It is two separate bodies: the right hook is X −54.2 to −5, the left X 25 to 54.2.

## Hook retention (2026-11-07)

Each hook is held by a toolless snap and a detent pin, with no fasteners and no change to the sides. Nothing else stops a hook, now that the lead gap splits the strap.

- **Snap barb, stops outward X.** The rear 7.2 mm of each bar (Z −15 to −7.8) is a tongue rooted at |X| 30, 1.6 thick, 17.8 long, ending at |X| 47.8. Its barb rises to Y 350, 1.1 mm past the slot ceiling at Y 348.9. The 90 deg catch face sits 0.2 mm from the web inner face, and the 4 mm ramp on the inboard side is the lead-in.
- **Detent pin, stops rearward Z.** The pin is now 0.8 tall, tip at |X| 50.9, with a 30 deg lead-in. It sits 0.6 mm into the 0.8 deep pocket, and 0.4 mm at the worst of the X float.
- **Lip, stops inward X.** Unchanged, 0.2 mm from the web outer face. The barb and lip straddle the web, so total X float is 0.4 mm, and together they stop the hook rocking when the pack pushes its inner end up.

**Fitting.** The old rear slide-in does not work on the assembled frame: corner bracket br17 (|X| 51 to 55, Y 325.6 to 353.6, Z −70 to −50) sits against the web rear edge, so the lip cannot get round it. Instead, from the frame side with the side shell off, line the hook up at Z −15 to 5 and push it inward along X through the web slot, inner end first. The ramp presses the tongue down, and it clicks when the barb clears the web. The hook only clicks with the pin in its pocket, so the click also confirms Z. It can be fitted with the pack in or out.

**Release.** With a thin blade from the rear, over the pack top, press the tongue down just inboard of the web, then pull the hook out sideways. The tongue has 1.9 mm of room above the pack and needs 1.1. The pack slides in and out under the strap, so a pack swap does not need this.

**Print.** Stand each hook on its rear face (Z −15). Load paths then lie in the layer plane, the tongue is on the bed and the bar bridges 21.7 mm from the root to the lip, with no supports. Untested.

| Check | Result |
| --- | --- |
| Hook to side, seated, and at ±0.2 mm X (`check_hook_fit.py`, 0.1 mm voxels) | No overlap |
| Hook rearward | Free for 0.2 mm, the pin is hitting the pocket by 0.4 mm |
| Hook outward | Free to 0.2 mm. Beyond that only the barb meets the side (Y 349 to 350) |
| Insertion path out past the frame side, boxes against `parts.json` | Clear of the frame and M5 heads. Boxes only, cables and the shell are not included |
| Hooks, solid | Right 2,927 mm3, 3.7 g. Left 1,726 mm3, 2.2 g |
| Tongue, hand calculation | 1.1 mm deflection: about 0.9 % strain, about 3 N at the barb (E 2 GPa) |

**Open.** Not printed, so the snap force, the pin and barb fits and the 21.7 mm bridge need a test print. Still no FEA of the hooks as cantilevers, and the bar is now 12 × 3 mm through the web. `parts.json` still shows each hook as a plain box without the barb or lip.
