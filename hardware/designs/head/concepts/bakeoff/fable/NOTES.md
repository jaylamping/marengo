# Entry "fable", round 3: marengo head after the Iron Giant (judged against MASTER_A / B / C only)

`python3 build_head.py` -> `out/*.stl`, `out/parts.json`, `out/head.step` (build123d 0.13, ~45 s).
Cranium 124 W x 130 D; crown z = 236, fin tip z ~253; jaw band 141 W, chin (jaw bottom) **z = 50**; ringed neck
z 12 to 50 above a collar lowered to 14 mm (r 75 / 68). Head crown-to-chin 186 mm. Materials: white head/collar,
grey1 jaw + bosses, grey2 fin, dark bezel/seams/core/neck, eye glow.

## The owner's four points and what changed

1. **Face "smushed".** Re-laid the vertical stack from MASTER_C (D = head depth) and MASTER_A: crown -> eye
   0.63 D (82 mm), eye -> jaw top at the front 0.46 D (60 mm; the ledge now slopes 10 mm downhill from the knee
   to the front as in C), jaw top -> chin 0.34 D (44 mm front, 54 mm at the sides). Straight walls now run 34 mm
   above the eye centre before the arch. The eyes: glow **28 mm** (0.23 W; A measures 0.21 W), ring 36 mm
   (0.29 W, A 0.29), spacing 55 mm (0.44 W, A 0.44). I used the smaller-display option (26-32 allowed): 28 mm
   keeps A's eye-to-head ratio within 0.02 W while leaving a 2 mm margin for the bezel over a 1.1" class panel.
2. **Mohawk.** Now a thin blade: 4.5 mm wide at the surface, sharp 0.7 mm edge; fine point 32 mm below the eye
   line (A: 0.25 W below the eyes), only 4.5 mm tall up the whole forehead, rising (t^3) in the last stretch to a
   21 mm peak at the top front of the dome (80 deg round the profile) that leans 40 deg back, so it stands ~0.14 W
   above the silhouette as in A/B; then it drops to a 1 mm ridge and ends at 110 deg (B shows nothing behind the
   peak, C shows only the brow curl).
3. **Chin assembly.** Rebuilt as a deep helmet-like piece (MASTER_C, A): 5.5 mm wall with a 4 mm floor, front
   face raked 12 mm forward to a crisp chin (r 6 at the chin, r 16 at the side corners), a dark horizontal seam
   groove across the front at 42 % of the band depth (A, C), a vertical rear edge at x = -58 with a diagonal cut
   to the chin (C), a wide side plate from the knee (x = -26) up to the pivot boss at eye height + 6 mm and
   0.85 D back (C), outlined with a 1.2 mm dark groove so it reads as a plate, and concentric bosses
   (r 8.5 / 5.5 / 2.8). The ear tabs are the plate's 4 mm-proud rounded ends (A).
4. **Neck.** The head is raised: jaw bottom z = 50 (was 22). A dark neck tube r 36 with three r 41 rings sits
   between the lowered collar (top z = 14) and the jaw floor (A: dark below the jaw; B: ring stack; C: tube).

## Measured ratios versus the masters

| feature | master | this entry |
|---|---|---|
| crown -> eye / D (C) | 0.64 | 0.63 |
| eye -> jaw top at front / D (C) | 0.47 | 0.46 |
| jaw top (front) -> chin / D (C) | 0.33 | 0.34 |
| head height / depth (C) | 1.43 | 1.43 |
| chin forward of the face / D (C) | ~0.10 | 0.09 |
| pivot boss: height above eye / D, back from front / D (C) | ~0.02, 0.85 | 0.05, 0.85 |
| knee (plate front edge) back from front / D (C) | ~0.55 | 0.70 |
| glow dia / W, ring dia / W, eye spacing / W (A) | 0.21, 0.29, 0.44 | 0.23, 0.29, 0.44 |
| band proud of the cranium per side / W (A) | ~0.06 | 0.056 |
| band depth at the front / W (A) | ~0.36 | 0.35 |
| fin above the dome silhouette / W (A) | ~0.14 | ~0.14 |
| fin width at the forehead / W (A) | ~0.04 | 0.036 |
| neck dia / W (B) | ~0.6 | 0.66 (rings), 0.58 (tube) |
| crown apex forward of plan centre / D (C) | ~0.08 | 0.08 |

Other shape choices from the masters: pointed-arch front profile (m 1.65) that is round-topped fore-aft
(m 2.15 front); the back of the head starts curving 4 mm below the eye line and falls in one long curve
(m 1.55) to the rear plate edge (C); plan rounds toward a circle at the apex.

## Character brief (inquisitive, brave, endearing)

Eyes dominate and sit high and wide with a dark ring and bright bevel (inquisitive); tall straight-sided head, a
single sharp blade and a forward chin on a deep jaw (brave); no brow, arch dome, rounded jaw corners, eyes a
touch larger than the film (endearing). Proportions are the film's, not chibi.

## Remaining flaws

- Jaw band is 141 mm wide (cranium 124), above the brief's 125-135 at that one point; A shows the band clearly
  wider than the cranium so I kept it. The ear tabs add 8 mm more.
- The knee of the side plate is further back than in C (0.70 D vs 0.55 D).
- From below the floor plate and neck hole are visible inside the band; the masters never show that angle.
- Eye glow 28 mm needs a display smaller than the 1.28" panel.
