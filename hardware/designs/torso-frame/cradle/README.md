# Pack cradle, rev B (as modelled)

**FIT-CHECK ONLY, not crash-rated** (Joey, 2026-11-07). The robot stays stationary and supported until the legs exist, and this is the stage to find out what breaks. Fall and tip-over capture from gate 2 packet 3 is deferred until then (see Rev B).

SolidWorks model of [CRADLE_SPEC.md](CRADLE_SPEC.md), in torso coordinates, 2026-11-06. The web inner face is at **|X| 48.0**, not the spec 47.5, so the pack slides with 0.5 mm per side. Gate 2 packet 3 reviewed (`../review/packet3/reviews/`). Print files are in [`../prints/`](../prints/README.md).
Native parts are in `J:\code\marengo\cad\torso\` (`marengo_torso_cradle_*`, named per `cad/manifests/cad-conventions.json`). `build_cradle.py`
rebuilds them, `check_overlaps.py`, `check_hook_fit.py` and `mass_estimate.py` give the numbers below, `render_cradle.py` makes `CRADLE_ASSEMBLED.png`.

## Parts

| Part | Qty | Volume, mm3 | Solid PETG, g | 4 walls / 15 % gyroid, g (estimate) |
| --- | --- | --- | --- | --- |
| `marengo_torso_cradle_side_right` (-X) | 1 | 55,948 | 71.1 | 62 |
| `marengo_torso_cradle_side_left` (+X) | 1 | 55,948 | 71.1 | 62 |
| `marengo_torso_cradle_top_strap` (two snap hooks) | 1 set | 4,771 | 6.1 | 6.1 |
| Set | | | 148.3 | 130 |

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
   Each web now has a closed slot, Y 345.5 to 348.9, Z −15.5 to 5.5, R0.4 corners. Each hook fits from the side (see Hook retention).
   The strap gained outer lips to |X| 54.2 and is 103.4 long across the webs. No extra fasteners.
5. **Added:** 3.5 × 3.5 root chamfers between web and feet (bottom full length, top on both sides of the slot; the rear one ends in a 45 deg slope so it prints without support).
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
| Hook to side | No overlap: 0.2 mm at the web, 0.4 mm in slot height, 0.5 mm at each slot end |

Both thumbscrews pull the pack toward their own web, so with the pack centred, tightening closes the 0.5 mm gaps by bowing the webs inward. That needs a check at assembly: it may want a spacer or a limit on tightening.

Not checked: strength (no FEA), and the thumbscrew head against the shell (assumed Ø18 × 6.5 for the frame check).
Print: see Print orientation below.

## Lead gap (2026-11-07)

The pack power leads (10 AWG silicone, 5.2 to 5.8 mm OD) cross the pack top along Z and did not fit the 5.1 mm left above the strap.
The strap now has a **30 mm gap at X −5 to +25**, through its full height and depth (`Lead_gap` in `build_cradle.py`), so the leads get Y 345 to 353.6 (8.6 mm) there.
It is two separate bodies: the right hook is X −54.2 to −5, the left X 25 to 54.2.

## Hook retention (rev B, 2026-11-07)

Each hook is held by a toolless snap and the slot itself, with no fasteners.

- **Snap barb, stops outward X.** The rear 6 mm of each bar (Z −15 to −9) is a tongue rooted at |X| 30, 1.6 thick, 17.8 long, ending at |X| 47.8. Its barb rises to Y 350, 1.1 mm past the slot ceiling at Y 348.9. The 90 deg catch face sits 0.2 mm from the web inner face, and the 4 mm ramp on the inboard side is the lead-in.
- **Slot ends, stop Z both ways.** The bar is full width (Z −15 to 5) from |X| 49 out to the lip, so it sits in the web with 0.5 mm to each slot end. The tongue tip stops at |X| 47.8, 1.2 mm short of that full-width part.
- **Lip, stops inward X.** Unchanged, 0.2 mm from the web outer face. The barb and lip straddle the web, so total X float is 0.4 mm, and together they stop the hook rocking when the pack pushes its inner end up.

**Fitting.** Corner bracket br17 (|X| 51 to 55, Y 325.6 to 353.6, Z −70 to −50) stays clear: the hook goes in well forward of it, at Z −15 to 5. From the frame side with the side shell off, line the hook up and push it inward along X through the web slot, inner end first. The ramp presses the tongue down, and it clicks when the barb clears the web. It can be fitted with the pack in or out.

**Release.** With a thin blade from the rear, over the pack top, press the tongue down just inboard of the web, then pull the hook out sideways. The tongue has 1.9 mm of room above the pack and needs 1.1. The pack slides in and out under the strap, so a pack swap does not need this.

**Print.** Stand each hook on its rear face (Z −15). Load paths then lie in the layer plane and the tongue is on the bed. The bar bridges 19 mm from the root at |X| 30 to the full-width part at |X| 49, over a 2 mm slit, with no supports.

| Check | Result |
| --- | --- |
| Hook to side, seated, and at ±0.2 mm X (`check_hook_fit.py`, 0.1 mm voxels) | No overlap |
| Hook in Z | Free to ±0.3 mm, meets the slot ends at about ±0.5 mm |
| Hook outward | Free to 0.3 mm. Beyond that only the barb meets the side (Y 349 to 350) |
| Insertion path out past the frame side, boxes against `parts.json` | Clear of the frame, br17 and M5 heads. Boxes only, cables and the shell are not included |
| Hooks, solid | Right 3,027 mm3, 3.8 g. Left 1,744 mm3, 2.2 g |
| Tongue, hand calculation | 1.1 mm deflection: about 0.8 % strain, about 2.4 N at the barb (E 2 GPa, 6 mm wide) |

**Open.** Nothing printed yet, so snap force, barb fit and the 19 mm bridge are for the test print. `parts.json` still shows each hook as a plain box without the barb or lip.

## Rev B (gate 2 packet 3, 2026-11-07)

Only what stopped a clean print, assembly or useful test was changed.

| Blocker | Change |
| --- | --- |
| Strap slot open to the rear edge cut the web from its top foot | Closed slot, Z −15.5 to 5.5, R0.4 corners. Web and top foot are continuous behind it, with a rear root chamfer added |
| 0.8 mm tongue slit under the bridge would fuse | Slit 2 mm: tongue Z −15 to −9, slit to Z −7 |
| Detent pin no longer has a job | Pin and pocket removed. The slot alone could not stop the hook rearward, since only the 12 mm bar sat in the web and it could slide about 8 mm, so the bar is now full width from \|X\| 49 to the lip |
| Side print orientation stood on a 3.5 × 178 mm sliver | Front edge (Z 37.5) down, see below |

The separate hook coupon was dropped (Joey, 2026-11-07): the hooks are about 5 g and serve as their own test.

### Print orientation

Front edge down, build direction −Z. The bed face is the web, front lip, pads and foot ends, about 9.5 mm wide along the 166 mm lip, plus the top foot, so no sliver. Both feet stay in the layer plane. The pack and frame bearing faces (ledge top, web inner face, foot mounting faces, M5 head faces, lip stop) are walls or top surfaces, so they carry no support scars, and the M3 insert pilots open straight onto the bed.

Supports go in three places only, all non-bearing, by enforcer: the forward edge of the bottom foot tongue (Z 21.5, 16 mm up), the underside of the thumbscrew boss, and the 2 mm of ledge past the lip. Auto supports were rejected: they went into the M3 pilots, M5 slots, strap slot and window arches. Those stay unsupported, so look at the window arches and M5 slot ends on the first print. Settings and times are in `../prints/README.md`.

### What each test should try to break

Deferred until the legs exist, not fixed here: rearward pack stop, pack capture after a printed part fails, independent battery containment, hook uplift at g loads.

| Test | Try to find |
| --- | --- |
| Thumbscrew clamp and web bowing | Tighten both with the pack in. Fable expects the second screw to bow its web about 1.0 mm, not 0.5, because the first pulls the pack to its own web. Measure the bow, see if the pack still slides, and find where the web or the insert gives |
| Hook click and release | Click and release force by hand against the 2.4 N estimate, whether the barb holds or rattles (0.2 mm gap), whether the blade release works over the pack, and how many cycles before the tongue whitens or cracks |
| Web and top foot stiffness | Push the bolted side by hand at pack height. Watch the web to top foot root, the slot ends and the 3.5 mm web beside the windows |
