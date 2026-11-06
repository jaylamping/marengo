# Measured part masses

Weighed by Joey on 2026-11-06, printed PETG parts as removed from the printer (supports off).
These are the masses `urdf/mass_overrides.json` uses. Add new parts here as they are weighed.

| Part | Infill | Weighed | CAD solid PETG (1.27 g/cm³) | Ratio |
|---|---|---|---|---|
| P1 shoulder link (L01) | 15% | 65 g | 170 g | 0.38 |
| P1 shoulder link (L01) | 100% | 160 g | 170 g | 0.94 |
| P2 shoulder link (L04B) | 15% | 62 g | 178 g | 0.35 |
| P2 shoulder link (L04B) | 100% | 166 g | 178 g | 0.93 |
| Roll spacer (PF03, 20 mm) | not recorded | 11 g | 25 g | 0.44 |

CAD volumes are from `urdf/cad_export.json`: P1 133.7 cm³, P2 140.4 cm³, spacer 19.6 cm³.

Notes:

- The left arm uses the 100% infill P1 and P2, so the URDF takes 160 g and 166 g.
- 100% prints come out about 6% under solid density. The generator now uses an effective 1.19 g/cm³
  for parts that aren't weighed yet (the upper arm), in place of 1.27.
- The spacer weighed 11 g, about 44% of solid, so it looks like a low-infill print. It sits in the
  clamped bolt stack between the roll RS03 output and P2, so it should be 100% infill (about 23 g)
  to resist crush and creep. The URDF uses the weighed 11 g until it is reprinted.
- 15% figures are kept for reference only; they are the earlier lighter prints.

Effect on `urdf/left_arm.urdf` (regenerated): total 2.620 kg to 2.568 kg. Shoulder pitch link
1.050 to 1.040 kg, shoulder roll link 0.608 to 0.582 kg, upper-arm yaw link 0.651 to 0.635 kg.
