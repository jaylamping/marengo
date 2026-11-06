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
- Filament PETG (Generic PETG HF profile), textured PEI plate, glue stick on smooth plate.
- Layer 0.20 mm, **infill 100 % rectilinear** (or 6 walls + 60 % gyroid if you want it faster), top/bottom 5 layers.
- Lying flat as placed, **no supports**, no brim needed.
- After printing: run an M5 drill or 5.5 mm bit through each hole if it is tight.

**Screws change with the thicker plate:** the 7 T-nut screws per plate are **M5 × 12** (not × 10); the 2 end-tap screws stay
**M5 × 35** (9 mm of thread in the tapped rail end) or M5 × 40 if you have them. Which side is which: with the plate on the front
face of the frame, the long arm goes UP the upper post and the R15 corner sits at the outside of the step.
