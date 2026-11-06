# Body shell, concept A

2026-11-06. Concept renders only. This is not a part and is not released for printing.

A white shell over the 2020 torso frame, from Joey's silhouette sketch: a broad chest that wraps the shoulder
pitch motors, a taper to the waist, and a longer stomach. The look takes cues from the film reference Joey
gave (white chest over darker mechanics) but the shapes are original. The renders are a parametric
mock-up, not SolidWorks; the real panels get modelled on Joey-PC once the look is agreed.

| Image | Shows |
| --- | --- |
| `SHELL_CONCEPT_HERO.png` | Three-quarter view with arms and a placeholder head |
| `SHELL_CONCEPT_VIEWS.png` | Front, side, back |
| `SHELL_CONCEPT_PROPORTIONS.png` | Full 5 ft figure: frame as built vs a +70 mm waist section |
| `SHELL_CONCEPT_PANELS.png` | X-ray of the frame and mount points; exploded print panels |

## Key dimensions (robot frame, X forward, Y left, Z up)

| Item | Value | Source |
| --- | --- | --- |
| Frame | 140 deep x 150 wide x 340 tall | `docs/kinematics.md` |
| Shoulder pitch axis | 424.9 mm above pelvis bottom; output faces at Y ±97 | `docs/kinematics.md` |
| Arm line | Y ±170.5 | `designs/left-arm/urdf/left_arm.urdf` |
| Chest shell, widest | 216 mm wide at the shoulders, 82 mm waist half-width | this concept |
| Chest depth | 110 mm front + 88 mm back of frame centre (198 mm) | this concept |
| Shoulder width over the deltoid caps | about 480 mm | this concept |
| Waist extension (option) | +70 mm between waist yaw and frame | closes the shoulder-height shortfall |

## Panels (Bambu P2S, 256 x 256 x 256 mm)

13 panels, all inside the build volume. The largest is the chest upper front at 216 x 224 x 110 mm.

- Chest: upper front, upper back, lower front, lower back (magnet-held service hatch for the Pi).
- Chest shield: separate raised front plate, so it can be printed in a different finish.
- Abdomen: three bands, each split front and back.
- Deltoid caps: one per arm. They ride on the roll actuator and move with the arm. Upper-arm and forearm
  shells belong to the arm design.

Proposed mounting: printed brackets on M5 T-nuts in the outer frame slots, M3 heat-set inserts in the panel
bosses, lapped seams.

## Open points

- The chest side is at Y 104–108 near the shoulder. With 15° inward roll the upper arm comes close to it;
  check against the arm thread's swept envelope before any panel is modelled.
- The deltoid cap clearance through the full roll and pitch range is unchecked.
- At 215 + 195 + 90 mm the arms read short against a 5 ft body in the proportion view.
- The +70 mm waist section changes the URDF torso height and needs a structural spacer, not just shell.

## Regenerating

`generator/` holds the three.js scene. Run `npm install three@0.170.0` in that folder, serve it over HTTP on
port 8765, then run `node shoot.js '[["hero","view=iso"]]'` (Playwright). Parameters: `ext` (waist extension, mm),
`view` (iso, front, side, back), `xray=1`, `explode=1`, `full=1`. `compose.py` builds the labelled sheets.
