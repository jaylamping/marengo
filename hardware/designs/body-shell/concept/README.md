# Body shell, concept B

2026-11-06. **Baseline LOCKED by Joey; rev B (2026-11-06): back deepened for the frame, waist ~193 with straight sides, see `../manufacturing/SHELL_BASELINE_REVB.png`.** (silhouette, proportions, shoulder cavity). Concept renders only: not a part and not released for printing. Iterations start from this baseline.

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
| `SHELL_CONCEPT_WIDTH.png` | Pec chest plate, no head, shoulders as built vs P1 +25 mm per side |
| `SHELL_CONCEPT_BLEND.png` | Latest: in-between side line, shoulders +38 mm per side via a wider 2020 frame (226 mm), waist +10 mm per side, rounded shoulder top and chest front |
| `SHELL_CONCEPT_FRAME.png` | X-ray of the shell over the two-box frame and gusset plates |
| `PROPORTIONS.md` | Proportion check of the baseline against ANSUR II men scaled to 5 ft |
| `SHELL_CONCEPT_CLEARANCE.png` | Baseline vs the arm-thread clearance fixes (socket r 93, shoulder half-width ≤155); Joey kept the baseline |

## Key dimensions (robot frame, X forward, Y left, Z up), baseline 2026-11-06

| Item | Value | Source |
| --- | --- | --- |
| Frame | Two-box: wide upper box (+38 mm per side, 226 wide) over the 150 wide lower box; 140 deep | arm thread, `designs/left-arm/envelope/TORSO_WIDE_VARIANT.md` |
| Shoulder pitch axis | 424.9 mm above pelvis bottom; RS03 output faces at Y ±135 | arm thread |
| Arm line | Y ±208.5 | arm thread |
| Chest shell | 324 mm at the shoulder point (rounded cap ~R40, trapezius slope to the neck); 278 mm 111 mm below the axis; 178 mm at the chest bottom | this concept |
| Waist (dark bands) | **~193 mm wide with straight sides (Joey 2026-11-06 07:19, rev B)**; was 156. 3 bands over the waist yaw RS03 | generator `wx=29`, `straight=1` (defaults) |
| Shoulder cavity | 78 mm radius around the pitch axis, floor at the RS03 face. r 76 is the minimum for full pitch (-50..180) with >= 3 mm at 0 and 5 deg inward roll | arm-thread sweep |
| Chest depth | ~124 mm front (pec) + 88 mm back (212 mm) | this concept |
| Shoulder collar | 114 mm OD, 86 mm ID, 2 mm flange, on the 8 unused outboard M4 holes (M4x10 button heads) | arm thread |
| Waist extension (option) | +70 mm between waist yaw and frame | brings shoulder height within 1 % of a 5 ft man |
| Proportions | Robot-stylised (Joey): heroic shoulders, slim waist | `PROPORTIONS.md` |

## Panels (Bambu P2S, 256 x 256 x 256 mm)

At the baseline width the chest panels exceed 256 mm across (upper ~310, lower ~279), so each splits at the centre line: 17 panels, all inside the P2S volume.

- Chest: upper front, upper back, lower front, lower back (magnet-held service hatch for the Pi).
- Chest shield: separate raised front plate, so it can be printed in a different finish.
- Shoulder collars: one per side, clamped under the pitch RS03 outer-ring bolts; the chest side panels lap onto them.
- Abdomen: three bands, each split front and back.

Upper-arm and forearm shells belong to the arm design. Proposed frame mounting: printed brackets on M5 T-nuts in
the outer frame slots, M3 heat-set inserts in the panel bosses, lapped seams.

## Open points

- **Manufacturing (2026-11-06):** split, mounts, doors, vents and the pre-print test ladder are in `../manufacturing/`. It found the back too shallow for the frame's rear corners and Joey approved the fix (back +10 mm, +20 at the frame bottom; generator defaults `bd`, `bdlo`, `nlo`, `wlo`). Earlier sheets in this folder show the pre-fix back.

- Shell fits the two-box frame (de5a006) with 15 mm or more to the gusset plates; panel mounts still to be placed on the new frame members.
- Upper-arm length card (240/200, 215/195 or 250/210) is open with Joey on the arm thread; the renders use 215/195.
- Collar M4 button heads stand 2.2 mm proud; the arm thread adds a local relief to P2's inner edge.
- Panels are sacrificial crash covers (load case: a trip-level fall from standing or a bench tip). Keep one fixed mounting interface per panel so variants (colour, shape, cooling and vents) can swap without frame changes.
- Exposed shoulder parts and the exploded-view frame are stand-ins.
- Armor-plate breakup and the real panels are SolidWorks work on Joey-PC.

## Regenerating

`generator/` holds the three.js scene. Run `npm install three@0.170.0` in that folder, serve it over HTTP on
port 8765, then run `node shoot.js '[["hero","view=iso"]]'` (Playwright). Parameters: `ext` (waist extension, mm),
`view` (iso, front, side, back), `fw` (frame widening per side, mm), `ysx` (P1 lengthening per side, mm), `wx` (waist widening per side, default 10), `sock` (socket radius, default 62), `topcap` (shoulder half-width reduction, default 0), `xray=1`, `explode=1`, `full=1`. `compose.py` builds the labelled sheets.
