# Power board carrier (rev B board)

Printed plate that holds the rev B power board (one 100 × 150 board, `hardware/electrical/pdb/design/FLOORPLAN_REVB.png`) in
front of the pack. Torso coordinates (X lateral, +X robot left; Y up; Z forward), mm. Picture: `CARRIER.png`.
Model: `build_carrier.py` (build123d) writes `marengo_torso_board_carrier.step` / `.stl`. Frame model item 16 (`pdb_carrier`).

| | |
| --- | --- |
| Plate | X −55 to 55, Y 190 to 340, Z 37.5 to 41.5 (4 mm), R4 corners, two R6 lightening windows X ±34, Y 206–256 and 274–324 |
| Mounting | rear face on the four cradle carrier pads (\|X\| 48 to 55.5, Y 195–205 and 325–335, M3 heat-set inserts at \|X\| 51.75); **4 × M3 × 8 countersunk** from the front at (±51.75, 200) and (±51.75, 330), flush, clear of the side cable band (\|X\| 50–55, Z 42–50) |
| Board | **6 × M3 F-F hex standoff 6 mm** at (±46, 194), (±46, 265), (±46, 336); **M3 × 6 button head** from the rear into each, head in a Ø6.5 × 2 counterbore so the rear stays flat over the 3 mm air gap to the pack sheet; board on top with 6 more M3 × 6. Board then sits Z 47.5 |
| Mass | ~49 g at 100 % PETG (38.3 cm³) |

Assembly order: standoffs onto the carrier first (their rear heads are partly under the pads), then the carrier onto the cradle,
then the board. To take the board off later, only the six front screws come out.

Print: flat on the rear face (Z 37.5 face down), PETG, 0.2 mm, 100 % infill, no supports (countersinks face up, counterbores
face down and bridge over 6.5 mm). **No 3MF yet:** printed parts go through the gate review first.

Checked: STL watertight; hole positions and radii sectioned at Z 38.5 / 40.5 / 41.3; countersink heads clear the standoff
hexes (8.3 mm centre to centre). Not checked: strength (no FEA; the board is ~250 g on six posts), and the gate-2 keep-out still
applies to the board, not the carrier: lower-post corner bracket heads at X ±46.5 to 51, Y 175–203 and 325–353, Z 55–65.
