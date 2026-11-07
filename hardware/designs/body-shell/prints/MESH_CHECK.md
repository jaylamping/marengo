# Pre-print mesh check (2026-11-07, `check_mesh.py`, files at main 8e4aa5f)

| File | Bodies | Fins/spikes (convex folds > 150°) | Open edges | Thinnest wall | Result |
| --- | --- | --- | --- | --- | --- |
| Fit kit: H3F L bracket | 1 | 0 | 0 | 3.0 mm | clean |
| Fit kit: H1 L bracket | 1 | 0 | 0 | 2.1 mm | clean |
| Fit kit: 4 caps | 4 (one per cap, intended) | 0 | 0 | 0.9 mm (cap rim) | clean |
| Fit kit: C13 pilot patch | 1 | 0 | 0 | 0.97 mm (the 1 mm skin under test) | clean |
| upper_front_L sternum-down | 1 | 0 | 0 | 0.04 mm at two screw-hole rims, 0.45 mm² total | OK to print; fix in the next rebuild |

Sliver triangles: there are 8 in the patch and 58 in the panel, all under 0.01 mm high. They are tessellation artifacts on edges and can't
print. The two thin webs on the panel are where two bottom-edge screw holes (X ≈ 50, Y ≈ 1 and Y ≈ 57, Z ≈ −60 in part coordinates)
almost break through the next face. Each is a ~0.1 mm web, thinner than one line, so it prints as a tiny notch in the hole rim. The panel
waits for the torso anyway: move those holes ≥ 1 mm from the face in the rebuild.

All-sides sheets, bed side included: `CHECK_fit_kit_all_sides.png`, `CHECK_upper_front_L_all_sides.png`.
