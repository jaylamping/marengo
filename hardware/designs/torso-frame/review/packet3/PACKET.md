# Gate 2 review packet 3: torso printed parts (pack cradle, strap hooks, board carrier)

Gate 2 = before any part is printed. Parts: the printed **pack cradle** (two C-shaped sides), the **top strap** (two snap hooks), and the
**power board carrier**. All PETG on a Bambu P2S. Torso coordinates: X lateral (+X robot left), Y up from the pelvis bottom, Z forward, mm.

## What they do

- 13S2P 21700 pack, 95 × 162 × 79 mm, 2.2 kg, box |X| ≤ 47.5, Y 183 to 345, Z −45 to 34, in the torso lower box between the 2020 posts
  (|X| 55 to 75). It slides out toward −Z through the rear door. A JK BMS (133 × 81 × 18) rides on its rear face (Z −65 to −45).
- Cradle sides bolt to the bottom side rails (top face Y 175) and under the shelf rails (bottom face Y 353.62) with M5 into T-nuts.
  The pack rests on a ledge, is stopped forward by a lip, and is held sideways by one M4 shouldered thumbscrew per side into a heat-set
  insert in the pack case (Y 252.6, Z −5). The strap hooks stop it lifting.
- The board carrier screws to four pads on the cradle front edge (Z 37.5) and carries a 100 × 150 power board (~250 g) on six 6 mm standoffs.
- Design load case for the robot: a ground-level tip-over from standing. Printed parts should fail before motors or the aluminium frame,
  but the pack must not come loose or be punctured.

## Files

- `data/cradle/`: README.md (as-built numbers, changes from spec, checks), CRADLE_SPEC.md (the spec it was built to), STEP + STL of both sides and the strap.
- `data/carrier/`: README.md, STEP + STL.
- `data/frame/`: parts.json (frame model, every member and part as boxes), README.md (frame), BUILD.md (assembly).
- `scripts/`: build_cradle.py, check_overlaps.py, check_hook_fit.py, mass_estimate.py, render_cradle.py (cradle, SolidWorks-built), build_carrier.py, draw_carrier.py (carrier, build123d).
- `views/`: CRADLE_ASSEMBLED.png, CARRIER.png.

## Checks done (by the author)

Box and voxel overlap checks of the cradle against every frame part; hook fit at 0.1 mm voxels; hand calculation of the snap tongue
(0.9 % strain, 3 N); mass from CAD volume; carrier STL watertight, hole positions and radii sectioned.

## Not done

No FEA of any part. Nothing printed. Thumbscrew bowing of the webs not quantified. Thermal: not checked (pack heat about 20 W steady,
36 W worst; PETG). Shell interference with the thumbscrew heads assumed (Ø18 × 6.5).

## Known open items

- Tightening both thumbscrews with the pack centred bows the 3.5 mm webs inward by up to 0.5 mm each.
- From Z −50 to 5 only a 4.7 mm strip joins each web to its top foot (strap slot).
- Hooks: 21.7 mm unsupported bridge in the print; hooks not modelled with barb and lip in parts.json.

## Questions

1. Load case: in a tip-over, the 2.2 kg pack loads the cradle in every direction. Which path fails first (feet M5 slots, web, ledge,
   lip, strap hooks, thumbscrews, inserts in the pack case), at roughly what g, and does the pack stay captured? Show the arithmetic.
2. Is the 4.7 mm web strip beside the strap slot adequate, given the strap and top foot load paths? Smallest fix if not.
3. Thumbscrew bowing: with 0.5 mm per side and two screws pulling outward-in, what happens to the webs and the pack clamp? Spacer, torque limit, or redesign?
4. Snap hooks: will they fit, hold and release as described (`data/cradle/README.md`, Hook retention), and print as described? Check the tongue arithmetic.
5. Board carrier: 4 mm PETG, 4 × M3 countersunk into inserts in printed pads, 6 standoffs counterbored from the rear. Adequate for a 250 g board in the load case and in vibration? Any interference the author missed (pads, side cable band |X| 50 to 55 Z 42 to 50, corner bracket heads at X ±46.5 to 51, Y 175 to 203 and 325 to 353, Z 55 to 65)?
6. Thermal: PETG webs 0.5 mm from the pack and the carrier 3 mm in front of it. Any risk at 36 W pack heat in a closed shell?
7. Print orientations and settings stated in the READMEs: anything that would make a test print a waste?
8. Anything in the files that contradicts another file (numbers, positions, fastener counts)?
