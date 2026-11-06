# Gate review packet: internal torso frame rev A with the locked power layout (2026-11-06)

## What this is
The internal 2020 aluminium frame of a 5 ft humanoid robot torso, plus everything packaged inside it. The frame is about to be
rebuilt from an existing single-box frame (photos in `data/reference/`) into a two-box stepped frame: a 150-wide lower (waist) box
and a 226-wide upper (shoulder) box joined at a step with 3 mm 6061-T6 gussets. Inside it: a custom 13S2P 21700 battery pack,
a power board and a logic board on one carrier, compute (Raspberry Pi 5, Jetson Orin Nano), fans, harness paths, and the two shoulder
RobStride RS03 actuators. This gate decides whether Joey cuts the posts, orders the gussets and builds the pack tray and board carrier.

Coordinates: torso frame, X lateral (+X robot left), Y up from the pelvis bottom, Z forward. Millimetres, grams.

## Files
- `data/README.md`: the frame spec (dimensions, change list, cut plan, shopping list, packaging check, open items).
- `data/parts.json`: every part, envelope and fastener as generated (boxes and cylinders with lo/hi corners, item numbers, masses).
- `data/bom.json`, `data/cutlist.md`: bill of materials and extrusion cut list, generated from `scripts/frame.py`.
- `data/gusset_plate_left.dxf`: the gusset cutting file (R15 at the X113/Y335 corner); `scripts/gusset_dxf.py` regenerates it.
- `data/custom-21700-pack.md`: the pack study (cells, voltage, mass, build, safety).
- `data/pdb_PLAN.md`, `data/pdb_design_README.md`, `views/pdb_FLOORPLAN.png`: power and logic board plan and floorplan rev A.
- `data/shell_manufacturing_README.md`: the body shell that bolts to the frame hardpoints (doors, pads, grilles).
- `data/torso_stepped_FEA_README.md`: the earlier frame FEA (tip-over load basis, gusset and bolt numbers).
- `data/robstride_README.md`: actuator electrical limits.
- `data/reference/*.jpg`: photos of the existing (built) frame bottom.
- `scripts/frame.py`: generator; rerun with `python3 frame.py` from `scripts/` to reproduce parts.json, bom.json, cutlist.md.
- `views/`: rendered sheets (assembled, exploded, fastener detail, dimensions, hardpoints, inside, packaging, change sheet) and
  2D packaging sections.

## Claims to check
1. Frame geometry: lower box 150 x 140, Y 155 to 353.62; shared 226 rails Y 353.62 to 373.62; upper box 226 x 140 to Y 495;
   shoulder axis Y 424.86; RS03 pitch output faces at X +-135. Cut plan: the four built 340 posts become 198.62 + 121.38.
2. Hardware: 32 corner brackets (24 x 2028 + 8 x 2020), 128 M5 screws and 128 T-nuts (10 mm; 100 slide-in + 50 drop-in),
   16 M4, M5 x 20 for the 20 shell hardpoint brackets, four 3 mm 6061-T6 gussets of about 40 g.
3. Pack: 13S2P Molicel P45B/P50B, 95 x 162 x 79, 2.2 kg, 46.8 V nominal, 54.6 V full, at |X| <= 47.5, Y 183 to 345, Z -45 to +34,
   on two printed tray rails on the lower post inner faces; it slides straight out the back through the shell's D2 battery door
   (|X| <= 60, Y 175 to 352). RS03/RS02 accept 24 to 60 V, so no regulator is needed.
4. Boards: power board ~100 x 95 (Y 245 to 340) and logic board ~100 x 55 (Y 190 to 243) on one printed carrier, rear face Z 37.5
   (3 mm air plus 0.5 mm Nomex over the pack lid at Z 34), component face Z 61.5, 8.5 mm to the frame front plane; 6 x M3 standoffs
   at X +-46, Y 194 / 265 / 336. No high-current plugs on either board: ring-lug pigtails on M4 terminals, routed over the pack top
   edge (8.6 mm under the shared rail), up the front corners, and down the side bands at Z 40 to 50.
5. Cooling: two 40 mm intakes in the front panel at X +-20, Y 264 to 304 and Y 193 to 233; two 40 mm exhausts in the upper rear door
   plane (provisional). 50 W brake resistor on the -X bottom side rail inner face, Y 155 to 180.
6. Every listed envelope in `parts.json` is free of interference with the frame, brackets, fasteners and each other, except where
   the README says otherwise.
7. The built frame is single 2020 rail throughout (confirmed by the owner); its bottom corners use printed gussets and metal angle
   brackets; printed foot tabs screw it to a bench stool.

## Checks done by the author
Coordinates and clearances computed from the generator, not CAD. Gusset FEA (in `torso_stepped_FEA_README.md`) was done on the
earlier step layout with assumed loads; nothing in this packet was load-tested. Pack, boards and fans are envelopes, not designs.
The board floorplan is rev A, schematic WIP. The tip-over basis assumes a 40 kg robot; the pack adds 2.2 kg in the lower box.

## Known open items (do not just restate them; say if any is worse than stated)
- Step joint capacity in a side fall: shear runs along the gusset slots, so the joint relies on T-nut friction (~1 kN assumed) against
  ~860 N per bolt; a positive end anchor per post is proposed but not designed.
- No slot-by-slot nut map or build order yet; slide-in nuts cannot reach some slots once the posts are on.
- Waist RS03 is 57 mm tall in a 40 mm waist bay (hip stage); a crossed-roller ring under the frame is proposed but not accepted.
- A third compute board has no home.

## Questions
1. Pack retention: in the ground-level tip-over (about 17 g peak per the FEA README), what holds the 2.2 kg pack in each direction
   (forward into the boards, backward against the D2 door, sideways, up)? Is a printed tray rail plus a magnet-closed door enough?
   What is the smallest change that makes it enough?
2. Electrical-mechanical safety: with 3 mm air plus 0.5 mm Nomex between the pack lid and the board carrier, and the boards' M4 ring
   terminals and pigtails in an 8.6 mm gap under the shared rail, what can short to the aluminium frame or to the pack in a crash,
   and does the single-point frame bond (1 Mohm || 10 nF) make any of that worse?
3. Interference: run your own check over `parts.json`. List every overlap or clearance under 2 mm between parts that must not touch,
   with part ids and coordinates.
4. Serviceability: can the pack come out through D2 without removing anything else? Can the boards be removed? Walk the steps.
5. Cooling: is the airflow path from the two intakes over the boards to the exhausts physically plausible with the pack, shelf and
   Jetson in the way? What is the worst hot spot?
6. Build: given the built frame in the photos, is the cut plan and the shopping list right? What would Joey find missing on the bench?
7. Step joint: is the open step-joint item above worse with the pack mass added low in the lower box? Should the gussets be ordered
   before the end anchor is designed?
8. What would make cutting the posts and ordering the gussets now a waste?
