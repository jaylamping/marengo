# Test pass 1 settings checklist

Open each file as a complete project, not a geometry import. Each 3MF is already sliced. The settings were compared key by key with the left P2 L04B project and every one matches.

Check these in Bambu Studio before you press print. If anything differs, stop and fix it before printing, because presets can reset values.

Each project lists its changes from the Bambu system presets, so Bambu Studio keeps them when it opens the file. Files from before 2026-11-07 midday didn't, and showed 15% infill on opening. **After opening a project, always confirm the plate type, walls and infill against this table, and that the print time matches `PRINT_ORDER.md`.** A preset can switch them when the project opens. If the print time is far off, a preset has changed something.

| Setting | Value |
| --- | --- |
| Printer | Bambu Lab P2S |
| Nozzle | 0.4 mm High Flow |
| Filament | Generic PETG @BBL P2S for every part that carries load or is under test. **Bambu PLA Basic @BBL P2S** for internal parts with no load and rig fixtures: the upper arm cover, dummy housing and dummy output. The elbow cable guard will be PLA too. Each print order names the material per plate |
| Bed | 70 °C for PETG, 40 °C for PLA, both on Cool Plate Super Tack |
| Plate | Cool Plate Super Tack, no glue |
| Layer height | 0.20 mm, first layer 0.20 mm |
| Walls | 6 |
| Infill | 100%, except `dummy_housing.3mf` at 40%, pattern shows as Rectilinear in the GUI (`zig-zag` in the file) |
| Top and bottom | 5 top layers, 3 bottom layers |
| Supports | Normal, auto, 45 degree threshold, not limited to the build plate |
| Support Z gap | 0.20 mm top and bottom |
| Interface layers | 2 top, 2 bottom |
| Nozzle | 255 °C for PETG, 220 °C for PLA |
| Flow ratio | 0.95 |
| Max volumetric speed | 12 mm³/s |
| Brim | **Outer brim only, 5 mm, on every plate.** It is saved in each file |

Before each print:

1. Dry the filament the same way for every part, and use the same spool if you can.
2. Keep the plate clean.
3. Don't rotate, move or auto-orient a part. The orientation is part of the test.
4. Write down the spool for each print.

No slicer warnings on any file. Times and weights are slicer estimates.
