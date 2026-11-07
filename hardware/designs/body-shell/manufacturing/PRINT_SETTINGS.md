# Body shell: exact print settings (DRAFT, confirm with coupons C1–C9 before the first real panel)

Bambu Lab P2S, Bambu Studio. Start from the system profiles named here and change only the lines listed. The 3MF project files
come with the print release after gate 2; this checklist is what they will contain.

## Every shell part (panels, doors, grilles, caps, brackets)

| Setting | Value | Why |
| --- | --- | --- |
| Printer / nozzle | P2S, 0.4 mm **High Flow** | fitted nozzle |
| Filament profile | Bambu PETG HF | tough in a fall, holds shape near warm actuators |
| Plate | Cool Plate SuperTack, no glue | Joey prints PETG on this plate; bed 60 °C from the PETG HF profile |
| Process profile | 0.20 mm Standard | |
| Wall loops | **6** (line width 0.40 mm) → solid 2.4 mm wall | the wall is all perimeters, no infill gaps |
| Sparse infill | 100 % (only bosses and pads have room for it) | bosses carry the screws |
| Seam | Aligned, painted to the rear/inside edge | keeps the zit off the show face |
| Supports | **Tree (auto)**, threshold 30°, "on build plate only" OFF | supports land inside the curved panels |
| Support Z gap | 0.25 mm top, interface 2 layers, interface spacing 0.5 mm | PETG fuses to PETG supports; this keeps them peelable |
| Brim | 5 mm outer brim on tall pieces (all chest panels, doors) | panels stand on the split edge |
| Part cooling | 40–60 % (profile default) | |
| Bed / nozzle temp | profile default (60 °C bed on SuperTack) | |

## Orientation (Joey 2026-11-06: clean outer face wins over print time)

| Part | On the plate | Time / support (SolidWorks STL, Bambu slice) |
| --- | --- | --- |
| upper_front L/R | sternum split face down | 11 h 05 m, 106 g |
| upper_back L/R | spine split face down | 11 h 30 m, 117 g |
| lower_front L/R | split face down | 8 h 40 m, 85 g |
| lower_back L/R | split face down (tilted 45° is 5 h 20 m / 42 g, only after a finish test) | 9 h 13 m, 97 g |
| screw caps (22) | roof down, all on one plate | minutes |

## After printing

- Peel tree supports from the inside; no sanding on the outer face should be needed.
- Heat-set inserts go in the brackets, never the panels.
- Weigh each part and add it to `MEASURED_MASSES.md`.

Unconfirmed until coupons: cap fit (C1), seam strip gap (C2), support Z gap, and the brim on the tallest panels.

## Ready-to-print files (sliced by the Bambu CLI, 2026-11-07; G-code inside, not yet opened in the Studio GUI)

In `hardware/designs/body-shell/prints/` (Git LFS) and on Joey-PC in `J:\code\marengo\cad\prints\shell\`:

| File | What | Time | Filament |
| --- | --- | --- | --- |
| `marengo_shell_fit_kit_upper_front_L.3mf` | **Print this first.** Real parts for that panel: H1 and H3 front left brackets, 4 screw caps, and the C13 pilot patch. Brackets at full settings, no brim; the patch shows whether the 1 mm skin over the pilot bulges on the outer face | 1 h 30 m | 41.6 g (1 g support) |
| `marengo_shell_upper_front_L_sternum_down.3mf` | First real panel, final settings | 9 h 23 m | 216 g (72 g support) |

Checklist when you print:
1. Load **PETG HF**; the files use the 0.4 **High Flow** nozzle profile (flow 0.92, 260 °C, 14 mm³/s from your PETG HF preset).
2. Cool Plate SuperTack, no glue.
3. Open, check the plate preview shows supports only on the inside, and print. Don't re-slice unless something looks wrong.
4. Known difference from the table above: the seam is plain "Aligned" (not painted). Harmless.
5. After the fit kit: screw an M2.5 plastite into the C13 pilots five times, check the outer face, and try the brackets and caps on the panel.
