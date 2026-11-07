# Marengo Head — Polish Pass Notes (after `fable_r3`)

`python build_head.py` -> `out/*.stl`, `out/parts.json`, `out/head.step` (build123d 0.13, ~35 s).
Part names (`head`, `crest`, `bezel`, `glow`, `jaw`, `seam`, `hinge`, `collar`, `core`, `neck`) and materials (`white`, `grey1`, `grey2`, `dark`, `eye`) are unchanged from `fable_r3`.

---

## Numbered List of Changes Made

1. **Removed the second spike behind the mohawk peak (`build_crest`, `P['crest_end_deg']`, `P['crest_h_back']`) — serves `MASTER_A`, `MASTER_B`, `MASTER_C`:**
   - **Why:** In `fable_r3`, two geometric bugs combined behind the mohawk peak (`u > u_peak`, `80°` to `110°`):
     1. The buried base-width formula `bb = b * (h + sink) / h` divided by `h` as `h` tapered from `21.0 mm` down to `0.5 mm`, blowing up the buried base width from `5.8 mm` to `37.0 mm`.
     2. Fitting a single smooth cubic BSpline (`loft(secs)`) across all 33 sections through the sharp cusp at `u_peak` (`80°`) and the slope break at `u_crown` (`96°`) caused severe spline ringing (Runge's phenomenon), shooting the tail at `x ≈ -6 mm` up to `z = 238.8 mm` (`8 mm` above the dome surface `z = 230.7 mm`) as a visible second triangle spike behind the main peak.
   - **What changed:** Clamped `h_eff = max(h, P['crest_h_face'])` in `bb = b * (h_eff + sink) / h_eff` so the buried base never blows up, ended the crest path at the crown (`crest_end_deg = 96.0`, `crest_h_back = 0.0`, since `MASTER_B` shows nothing behind the peak and `MASTER_C` shows only the brow curl), and split the loft at `u_peak` into `loft(U_front) + loft(U_back)` sharing the exact peak section. The front blade and backward-leaning `21 mm` peak at `80°` are untouched, while the second spike is completely gone.

2. **Moved the jaw side-plate "knee" forward from `0.70 D` to `0.55 D` (`P['knee_x']`, `ledge_at`, `band_profile`, `build_jaw`) — serves `MASTER_C`:**
   - **Why:** In `fable_r3`, `knee_x = -26.0 mm` placed the front-bottom knee of the side-plate at `(hd - knee_x) / D = (65 - (-26)) / 130 = 0.70 D` back from the front of the cranium, whereas the barn scene side view (`MASTER_C`) places this knee near mid-depth at `~0.55 D`.
   - **What changed:** Moved `knee_x` from `-26.0 mm` to `-6.5 mm` (`(65 - (-6.5)) / 130 = 0.550 D`) and referenced `P['knee_x']` in `ledge_at(x)` and `band_profile()` (replacing the hardcoded `-30.0`) so the `10 mm` downhill ledge slope begins cleanly at the knee (`x = -6.5 mm`, `z = 104.0 mm`) and the side-plate sweeps up and back from `0.55 D` to the rounded pivot end. Clipped the upper ear-pad extrusion (`pad`) laterally to `y_pad = side_y(hinge_x + strap_r, ...)` so its front edge stays flush with the angled strap while its outer `Y` face in front view remains vertical at `141.3 mm` overall width.

3. **Verified and kept the jaw band width (`136.1 mm` across the band, `141.3 mm` across the upper ear pads) — serves `MASTER_A`:**
   - **Why:** Checked the jaw width against the film still (`MASTER_A`) and `fable_r3` measurements. In `build_head.py`, the jaw band itself (`z < 115 mm`) has half-width `hw + jaw_gap + jaw_wall = 61.0 + 1.5 + 5.5 = 68.0 mm` (`136.1 mm` full width), standing `7.0 mm` (`0.056 W`) proud of the cranium per side—matching `MASTER_A`'s measured `~0.06 W` per side. The `141.3 mm` bounding-box width of `jaw.stl` is across the `4 mm`-proud upper ear tabs (`pad`) near the pivot bosses. Reducing the `1.5 mm` shadow gap or `5.5 mm` wall thickness would make the bucket jaw read too thin/flush against the cranium in `MASTER_A`, so the band width is kept unchanged.

4. **Lowered the pivot boss center height from `0.05 D` to `0.02 D` above eye level (`P['hinge_z']`) — serves `MASTER_C`, `MASTER_A`, and `BRIEF.md`:**
   - **Why:** `fable_r3/NOTES.md` measured the pivot boss height above the eye center in `MASTER_C` as `~0.02 D` (`+2.6 mm`), and `BRIEF.md` specifies the ears sit *"at about eye height"*, but `fable_r3` set `hinge_z = EYE_Z + 6.0` (`0.05 D`), pushing the top of the strap/ear tab (`z = 176.0 mm`) above the top of the eye bezels (`z = 172.0 mm`).
   - **What changed:** Set `hinge_z = EYE_Z + 2.5` (`0.02 D` above `EYE_Z`), aligning the pivot boss and ear tabs with eye level as in `MASTER_C` and `MASTER_A`.

5. **Eliminated the horizontal loft bulge/crease across the cheeks and side walls at eye level (`build_cranium`) — serves `MASTER_A`, `MASTER_B`, `MASTER_C`:**
   - **Why:** In `fable_r3`, `build_cranium()` used a single ghost section `secs = [sec(zs - 10, hw, hd)]` at `z = 140.0 mm` below `zs = 150.0 mm`. Because only position (not vertical tangent) was clamped at the start of the 20-section BSpline loft, the loft bowed outward by `+1.08 mm` per side at `z = 142.6 mm` and kinked by `43°` (`|nz| = 0.685`) at `z = 140.0 mm`, producing a visible horizontal crease across the face and side walls right along the lower rim of the eyes (`ROUND 3` renders).
   - **What changed:** Replaced the single ghost section with two closely-spaced sections `[sec(zs - 4, hw, hd), sec(zs - 2, hw, hd)]` (and extruded `walls` to `zs - 4`), clamping the vertical tangent at the wall-to-dome transition and reducing the normal kink from `43°` to `< 0.7°` (`|nz| = 0.013`). The face and side walls now read as clean, uninterrupted vertical plates up to `wall_top` (`EYE_Z + 34 mm`), strengthening the Giant's open, inquisitive expression.

6. **Trimmed the bottom of the inner dark `core` so it no longer pokes through the curved jaw floor from below (`build_dark`) — serves `MASTER_A`:**
   - **Why:** In `fable_r3`, `core` was cut flat at `z = CHIN_Z + 3 = 53.0 mm`, while the jaw's underside (`bottom = keep & across`) arcs upward by `6 mm` laterally (`bottom_sag`) and `8 mm` aft (`bottom_back_z = 58.0 mm`). Consequently, the lower-left and lower-right edges of `core` protruded below the jaw floor, creating two curved black gashes on the underside of the jaw in the `from below` view (`ROUND 3 from below`).
   - **What changed:** Raised the bottom cut of `core` from `CHIN_Z + 3` (`53.0 mm`) to `CHIN_Z + 9` (`59.0 mm`), which sits cleanly above the curved jaw floor everywhere while still overlapping the dark neck column (`z <= 62.0 mm`) and preserving the dark shadow line along the top jaw ledge (`z = 94..102 mm`).

---

## Measured Ratios Versus the Masters

| feature | master | `fable_r3` | polished |
|---|---|---|---|
| crown -> eye / D (C) | 0.64 | 0.63 | 0.63 |
| eye -> jaw top at front / D (C) | 0.47 | 0.46 | 0.46 |
| jaw top (front) -> chin / D (C) | 0.33 | 0.34 | 0.34 |
| head height / depth (C) | 1.43 | 1.43 | 1.43 |
| chin forward of the face / D (C) | ~0.10 | 0.09 | 0.09 |
| pivot boss: height above eye / D, back from front / D (C) | ~0.02, 0.85 | 0.05, 0.85 | **0.02, 0.85** |
| knee (plate front edge) back from front / D (C) | ~0.55 | 0.70 | **0.55** |
| glow dia / W, ring dia / W, eye spacing / W (A) | 0.21, 0.29, 0.44 | 0.23, 0.29, 0.44 | 0.23, 0.29, 0.44 |
| band proud of the cranium per side / W (A) | ~0.06 | 0.056 | 0.056 |
| band depth at the front / W (A) | ~0.36 | 0.35 | 0.35 |
| fin above the dome silhouette / W (A) | ~0.14 | ~0.14 | ~0.14 |
| fin width at the forehead / W (A) | ~0.04 | 0.036 | 0.036 |
| neck dia / W (B) | ~0.6 | 0.66 (rings), 0.58 (tube) | 0.66 (rings), 0.58 (tube) |
| crown apex forward of plan centre / D (C) | ~0.08 | 0.08 | 0.08 |

---

## Suggestions Not Applied (Taste / Hardware Trade-offs)

- **Eye display diameter (`28 mm` vs `30–32 mm`):** `fable_r3` uses a `28 mm` glowing eye diameter (`eye_ri = 14.0 mm`, `0.23 W`) to match `MASTER_A`'s `0.21 W` ratio rather than the `>= 30 mm` opening for a bare `1.28"` display in `BRIEF.md`. If hardware packaging requires the full `30 mm` active aperture without masking the perimeter by `1 mm`, `eye_ri` can be increased to `15.0 mm` (`eye_ro = 19.0 mm`), though `28 mm` is closer to the film still's proportion.
- **Concave fillet at the jaw knee (`strap_profile`):** The intersection of the rising side-plate front edge and the sloping jaw ledge at `knee_x = -6.5 mm` forms a crisp geometric knee. Adding a small `4–6 mm` 2D fillet radius in `XZ` at `(knee_x, ledge_z)` is a stylistic option if a softer sheet-metal transition is preferred.
