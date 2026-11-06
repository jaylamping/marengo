# Body shell, concept B

2026-11-06. Concept renders only. This is not a part and is not released for printing.

A white shell over the 2020 torso frame, from Joey's silhouette sketch: a broad chest, a taper to the waist,
and a longer stomach. Concept B (Joey, 2026-11-06): no shell on the shoulders. At shoulder height the torso shell
spans from one pitch RS03 face to the other (Y ±97) and mounts by a collar under each RS03's outer-ring bolts;
P1, the roll RS03, P2 and everything outboard stay exposed. Concept A (deltoid caps) is in git history. The look takes cues from the film reference Joey
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
| Chest shell, widest | 194 mm at the shoulders (RS03 face to face), 82 mm waist half-width | this concept |
| Chest depth | 108 mm front + 88 mm back of frame centre (196 mm) | this concept |
| Shoulder collar | 114 mm OD, 86 mm ID, about 3 mm, under the RS03 outer-ring bolts | this concept |
| Waist extension (option) | +70 mm between waist yaw and frame | closes the shoulder-height shortfall |

## Panels (Bambu P2S, 256 x 256 x 256 mm)

13 panels, all inside the build volume. The largest is the chest upper front at 194 x 224 x 108 mm.

- Chest: upper front, upper back, lower front, lower back (magnet-held service hatch for the Pi).
- Chest shield: separate raised front plate, so it can be printed in a different finish.
- Shoulder collars: one per side, clamped under the pitch RS03 outer-ring bolts; the chest side panels lap onto them.
- Abdomen: three bands, each split front and back.

Upper-arm and forearm shells belong to the arm design. Proposed frame mounting: printed brackets on M5 T-nuts in
the outer frame slots, M3 heat-set inserts in the panel bosses, lapped seams.

## Open points

- The RS03 outer-ring bolts also hold the motor to its bracket. The collar is not final until the arm thread's
  envelope README (`designs/left-arm/envelope/`) confirms which bolts, screw length, thread engagement, and the extra
  grip a 2–3 mm printed flange needs. Printed plastic under a structural bolt can creep; check that it is acceptable.
- P1 and the roll RS03 must never sweep into a plane about 3 mm outboard of the pitch housing face.
- The chest side is at Y ±97 near the shoulder. Check the upper arm at 15° inward roll against it with the swept envelope.
- The exposed shoulder parts in these renders are simple stand-ins, not the real P1/P2 geometry.
- At 215 + 195 + 90 mm the arms read short against a 5 ft body in the proportion view.
- The +70 mm waist section changes the URDF torso height and needs a structural spacer, not just shell.

## Regenerating

`generator/` holds the three.js scene. Run `npm install three@0.170.0` in that folder, serve it over HTTP on
port 8765, then run `node shoot.js '[["hero","view=iso"]]'` (Playwright). Parameters: `ext` (waist extension, mm),
`view` (iso, front, side, back), `xray=1`, `explode=1`, `full=1`. `compose.py` builds the labelled sheets.
