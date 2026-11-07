# Torso frame prints

## Temporary gussets, 6 mm PETG (bench stand-in for the 3 mm 6061 plates)

Joey 2026-11-06 20:45Z. Same outline and holes as `../gusset_plate_left.dxf` (R15 corner, 9 × 5.5 mm holes, end-tap holes at X 65 / 103),
6 mm thick instead of 3. Made by `../generator/gusset_print.py`. **Bench use only** (mounting and moving the arms); swap in the
aluminium plates before falls or hard motion. Not gate reviewed.

| File | What |
| --- | --- |
| `gusset_temp_x4_6mm.3mf` | All four on one plate: 2 left, 2 right (mirrored). Open in Bambu Studio, it lands flat |
| `gusset_temp_left_6mm.stl`, `gusset_temp_right_6mm.stl` | Single plates |

Each plate 58 × 140 × 6 mm, about 38 g at 100 %.

**Bambu P2S settings** (0.4 mm High Flow nozzle):
- Filament PETG (Generic PETG HF profile), Cool Plate SuperTack, no glue.
- Layer 0.20 mm, **infill 100 % rectilinear** (or 6 walls + 60 % gyroid if you want it faster), top/bottom 5 layers.
- Lying flat as placed, **no supports**, no brim needed.
- After printing: run an M5 drill or 5.5 mm bit through each hole if it is tight.

**Screws change with the thicker plate:** the 7 T-nut screws per plate are **M5 × 12** (not × 10); the 2 end-tap screws stay
**M5 × 35** (9 mm of thread in the tapped rail end) or M5 × 40 if you have them. Orientation: all four plates are really the same part (a mirrored plate is just the other one flipped over), so any plate fits any corner. On the frame, the narrow arm runs UP the upper post (X ±93 to 113), the wide bottom sits on the lower post (X ±55 to 75), and the R15 rounded corner is at the outside of the step.

## Cradle and board carrier, fit-check set

**Fit-check only, not crash-rated.** Light settings for everything (Joey 2026-11-07), since nothing in this set depends on print strength yet. Made by `build_prints.py` with the Bambu Studio CLI: P2S 0.4 mm nozzle, Joe PETG HF, Cool Plate SuperTack, 0.28 mm layers, 3 walls, 15 % gyroid.

| File | What | Orientation | Supports | Time | Filament |
| --- | --- | --- | --- | --- | --- |
| `marengo_torso_cradle_sides_x2.3mf` | Both cradle sides on one plate | Front edge down (Z 37.5 on the bed) | Tree, 3 g, three places | 3 h 3 min | 104 g |
| `marengo_torso_cradle_hooks.3mf` | Both snap hooks | Rear face down (Z −15 on the bed) | None | 22 min | 5 g |
| `marengo_torso_board_carrier.3mf` | Power board carrier | Rear face down (Z 37.5 on the bed) | None | 1 h 1 min | 34 g |

Print the hooks first. They take about 20 minutes and show whether the 2 mm slit stayed open, the 19 mm bridge held and the tongue flexes. The click itself needs a side's web slot, so check that once a side is printed.

The sides use tree supports in three places only (bottom foot tongue, thumbscrew boss, ledge tip), see `../cradle/README.md`. The plate has a 5 mm brim. Pull the supports off the boss and tongue, and check the M3 pilots and M5 slots are clear.
