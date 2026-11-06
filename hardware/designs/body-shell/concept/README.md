# Body shell, concept B

2026-11-06. Concept renders only. This is not a part and is not released for printing.

A white shell over the 2020 torso frame, from Joey's silhouette sketch: a broad chest, a taper to the waist,
and a longer stomach. Concept B (Joey, 2026-11-06): no shell on the arm's shoulder parts; a male build with a heavy chest block,
square shoulders, a strong V-taper and a slim dark waist (Joey's markup and references). The armor-plate
breakup Joey described is not in these renders: it gets done in SolidWorks once the silhouette is agreed. Around each pitch joint the shell steps in to the RS03 face (Y ±97)
inside a 62 mm radius socket and mounts by a collar under the RS03's outer-ring bolts; P1, the roll RS03, P2
and everything outboard stay exposed. Concept A (deltoid caps) is in git history. The look takes cues from the film reference Joey
gave (white chest over darker mechanics) but the shapes are original. The renders are a parametric
mock-up, not SolidWorks; the real panels get modelled on Joey-PC once the look is agreed.

| Image | Shows |
| --- | --- |
| `SHELL_CONCEPT_HERO.png` | Three-quarter view, exposed shoulders, placeholder head |
| `SHELL_CONCEPT_VIEWS.png` | Front, side, back |
| `SHELL_CONCEPT_PROPORTIONS.png` | Full 5 ft figure: frame as built vs a +70 mm waist section |
| `SHELL_CONCEPT_PANELS.png` | Shoulder close-up of the collar; exploded print panels |

## Key dimensions (robot frame, X forward, Y left, Z up)

| Item | Value | Source |
| --- | --- | --- |
| Frame | 140 deep x 150 wide x 340 tall | `docs/kinematics.md` |
| Shoulder pitch axis | 424.9 mm above pelvis bottom; output faces at Y ±97 | `docs/kinematics.md` |
| Arm line | Y ±170.5 | `designs/left-arm/urdf/left_arm.urdf` |
| Chest shell | 242 mm at the shoulders; sides ±90 from 100–200 mm below the shoulder axis; 158 mm at the chest bottom | this concept |
| Waist (dark bands) | 136 mm wide, 3 bands over the waist yaw RS03 | this concept |
| Shoulder socket | 62 mm radius around the pitch axis, floor at the RS03 face (Y ±97.5); P1/roll sweep is ~57 mm | this concept |
| Chest depth | 108 mm front + 88 mm back of frame centre (196 mm) | this concept |
| Shoulder collar | 114 mm OD, 86 mm ID, about 3 mm, under the RS03 outer-ring bolts | this concept |
| Waist extension (option) | +70 mm between waist yaw and frame | closes the shoulder-height shortfall |

## Panels (Bambu P2S, 256 x 256 x 256 mm)

13 panels, all inside the build volume. The largest is the chest upper front at 242 x 228 x 108 mm (14 mm margin).

- Chest: upper front, upper back, lower front, lower back (magnet-held service hatch for the Pi).
- Chest shield: separate raised front plate, so it can be printed in a different finish.
- Shoulder collars: one per side, clamped under the pitch RS03 outer-ring bolts; the chest side panels lap onto them.
- Abdomen: three bands, each split front and back.

Upper-arm and forearm shells belong to the arm design. Proposed frame mounting: printed brackets on M5 T-nuts in
the outer frame slots, M3 heat-set inserts in the panel bosses, lapped seams.

## Open points

- Collar mounting (from the arm thread): the 8 unused outboard M4 holes on the pitch RS03's 98 mm bolt circle, new M4x10
  button heads, 2–3 mm flange, bore at least 72 mm. P1 stays beyond Y 101.75 from r 24 to 54, so the flange must stay inside that.
- Sides are pulled in to Y ±90 from 100–200 mm below the shoulder axis, as the arm thread asked (at least 5 mm at 5° inward roll).
  Re-check once Joey picks the inward roll stop (5° or 15°); see `../../left-arm/envelope/`.
- The exposed shoulder parts in these renders are simple stand-ins, not the real P1/P2 geometry.
- At 215 + 195 + 90 mm the arms read short against a 5 ft body in the proportion view.
- The +70 mm waist section changes the URDF torso height and needs a structural spacer, not just shell.

## Regenerating

`generator/` holds the three.js scene. Run `npm install three@0.170.0` in that folder, serve it over HTTP on
port 8765, then run `node shoot.js '[["hero","view=iso"]]'` (Playwright). Parameters: `ext` (waist extension, mm),
`view` (iso, front, side, back), `xray=1`, `explode=1`, `full=1`. `compose.py` builds the labelled sheets.
