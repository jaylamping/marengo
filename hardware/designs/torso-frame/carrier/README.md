# Power board carrier (rev B board)

Printed plate that holds the rev B power board (one 100 × 150 board, `hardware/electrical/pdb/design/FLOORPLAN_REVB.png`) in
front of the pack. Torso coordinates (X lateral, +X robot left; Y up; Z forward), mm. Picture: `CARRIER.png`.
Model: `build_carrier.py` (build123d) writes (importing it, e.g. from `draw_carrier.py`, also writes them) `marengo_torso_board_carrier.step` / `.stl`. Frame model item 16 (`pdb_carrier`).

| | |
| --- | --- |
| Plate | X −59 to 59, Y 184 to 346, Z 37.5 to 41.5 (4 mm), R4 corners, two R6 lightening windows X ±38, Y 202–257 and 273–328. Edge ligaments ≥ 4.1 mm at every hole (gate 2 packet 3: were 0.15 and 0.75 mm) |
| Mounting | rear face on the four cradle carrier pads (\|X\| 48 to 55.5, Y 195–205 and 325–335, M3 heat-set inserts at \|X\| 51.75); **4 × M3 × 8 countersunk** from the front at (±51.75, 200) and (±51.75, 330), flush, clear of the side cable band (\|X\| 50–55, Z 42–50) |
| Board | **6 × M3 F-F hex standoff 6 mm** at (±46, 194), (±46, 265), (±46, 336); **M3 × 4 button head** from the rear into each (2.5 mm thread engagement), head in a Ø6.5 × 2.5 counterbore (0.85 mm recess margin) so the rear stays flat over the 3 mm air gap; board on top with 6 × **M3 × 4** (2.4 mm engagement through a 1.6 mm PCB). Tips stay 1.1 mm apart inside a through-tapped 6 mm F-F standoff (gate 2: M3 × 6 both ends would have collided by 2.4 mm). Board then sits Z 47.5 |
| Mass | ~54 g at 100 % PETG (42.4 cm³) |

Assembly order: standoffs onto the carrier first (their rear heads are partly under the pads), then the carrier onto the cradle,
then the board. To take the board off later, only the six front screws come out.

Print: flat on the rear face (Z 37.5 face down), PETG, 0.2 mm, 100 % infill, no supports (countersinks face up, counterbores
face down and bridge over 6.5 mm). **No 3MF yet:** gate 2 packet 3 (2026-11-07) was NO-GO; this is the rev with the carrier findings fixed.

Checked: STL watertight; hole positions and radii sectioned at Z 38.5 / 40.5 / 41.3 (rev 1); countersink heads clear the standoff
hexes (8.3 mm centre to centre); grown plate clear of every part in the frame model (box check). The rear face also bears on the cradle
front lips (|X| 42 to 47.5), not only on the pads. Not checked: strength (no FEA; the board is ~250 g on six posts), and the gate-2 keep-out still
applies to the board, not the carrier: lower-post corner bracket heads at X ±46.5 to 51, Y 175–203 and 325–353, Z 55–65.
