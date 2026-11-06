# Body shell: manufacture, mounting and assembly plan

2026-11-06. Thread "Manufacture and assemble the body shell". **Study, not a released part.** The geometry is the locked
concept baseline (`../concept/`) made solid with a 2.4 mm wall, plus the back clearance fix Joey approved on 2026-11-06 (section 1).
Nothing here is cleared to print: gate-2 review comes first (section 9).

Evidence labels follow the project convention: **script check** (ran here, reproducible), **estimate**, **assumption**, **unresolved**.

| Sheet | Shows |
| --- | --- |
| `SHELL_BASELINE_REVB.png` | Rev B baseline: deepened back, ~193 mm straight-sided waist (Joey 2026-11-06) |
| `SHELL_WAIST_OPTIONS.png`, `SHELL_WAIST_STRAIGHT.png` | Waist width studies behind rev B |
| `SHELL_MFG_SPLIT.png` | The proposed split: assembled front/back, exploded front/back |
| `SHELL_MFG_PLATES.png` | All 24 prints, each in its chosen orientation inside a 256 mm P2S volume |
| `SHELL_MFG_MOUNTS.png` | X-ray: the 20 hardpoint brackets on the frame, and the cooling airflow path |
| `SHELL_MFG_BACKFIX.png` | The frame clash found in the baseline and the fix |
| `SHELL_MFG_SPINEPACK.png` | Back crush pads (first pass) |
| `split_report.json` | Per-piece print size, mass, support and time estimates |
| `frame_clearance.json` | Frame-to-shell clearance, row by row |

## 1. Finding: the baseline back is too shallow for the frame (script check)

`frame_clearance.py` cuts the shell every ~3 mm in height and measures the gap from the inside of the 2.4 mm wall to the two-box
frame and gusset plates (outline from `../../torso-stepped/README.md`).

| Where | Locked baseline | With the fix |
| --- | --- | --- |
| Lower-box rear corners, bottom 100 mm | **through the wall by up to 11.5 mm** | 5.6 mm at the bottom row, 9 mm or more above it |
| Gusset-plate rear corners (step) | 0.3 to 1 mm | 7.8 mm |
| Across the back of the upper box | 6 to 8 mm | 10 to 17 mm |
| Front | 14 mm or more | unchanged |

The earlier x-ray check reported 15 mm or more to the gusset plates. This row-by-row check includes the frame and plate corners, which are the tight spots.

**Fix (generator params `bd=10&bdlo=10&nlo=1.0&wlo=4`):** back 10 mm deeper, 20 mm deeper at the frame bottom; bottom of the chest
4 mm wider per side and slightly squarer. The front, shoulders, socket, collar and waist bands are untouched. Chest depth goes from
212 to 222 mm. All renders and numbers in this folder use the fix. **Approved by Joey 2026-11-06** and now the generator default; pass `bd=0&bdlo=0&nlo=0&wlo=0` for the old baseline.

**Cross-thread consequence:** the arm thread's tip-over FEA assumes 40 mm of shell and link crush. The shell has 8 to 17 mm of gap to the
frame at the back and sides (14 to 55 mm at the front). A backward fall onto the shell alone gives far less crush than assumed, so
either the FEA uses ~10 mm at the back (peaks scale up about 4x) or the shell adds crush bumpers (TPU, section 6).

## 2. Split strategy

Two kinds of seam, treated differently:

- **Print splits** exist only because a part is wider than the P2S. They are bolted, not glued, on inner splice plates, so a
  cracked piece is reprinted on its own. They sit in natural grooves (sternum, spine) so they read as design lines.
- **Service splits** are where a unit comes off the robot. Each service unit has one fixed interface (hardpoint holes plus two
  locating pins), so colour, shape and vent variants swap without touching the frame.

Every seam gets a deliberate 1 mm V shadow gap. A designed reveal hides the ±0.3 mm a printed panel will be out, where a flush butt
joint would show every error. Laps are 8 mm, with the upper or outer piece over the lower one so it sheds dust and doesn't catch.

### Pieces (script check: `split_check.py`)

All 24 prints fit inside 250 mm usable (256 mm bed less 3 mm a side). The tallest is the upper back half at 246 mm.

| Unit | Prints | Largest print (mm) | Notes |
| --- | --- | --- | --- |
| U1 upper front | 2 (L/R pec plates, sternum split) | 228 × 124 × 154 | Chest shield covers the sternum seam (to model) |
| U2 upper back | 2 (spine split) | 180 × 157 × 246 | Frames the compute door |
| U3 lower front | 2 | 140 × 107 × 167 | |
| U4 lower back | 2 | 140 × 97 × 167 | Frames the battery door |
| D1 compute door | 1 | 121 × 120 × 3 | Pi 5, CAN HAT, shelf. Vent/fan variants |
| D2 battery door | 1 | 177 × 120 × 7 | Custom 13S2P pack slides out the back; spans the U2/U4 seam |
| D3 shoulder hatches | 2 | 101 × 34 × 52 | Pitch RS03 connectors and harness entry |
| V1 side vent grilles | 2 | 130 × 37 × 117 | Intake. Swappable cooling variants |
| C shoulder collars | 4 halves | 114 × 57 × 2 | Split front/back so they come off with the arm fitted |
| A1–A3 abdomen bands | 6 (front/back) | 156 × 69 × 55 | Clip to a carrier on H6 |

**Mass and time (estimate):** 1.23 kg of PETG for the shell, 22 g support, 32 to 53 h of printing at 6 to 10 mm³/s effective.
Add about 160 g for hardpoint brackets and 60 g of fasteners and magnets: roughly **1.45 kg** on the robot. The 2.4 mm wall is
the main lever: 2.0 mm saves about 200 g (coupon C7 decides). These are volume-based estimates; Bambu Studio slices replace them
at stage 3.

Not yet in the pipeline: chest shield plate, splice plates, bosses, laps and ribs (all SolidWorks work).

## 3. Join methods compared

| Method | Use | For | Against | Verdict |
| --- | --- | --- | --- | --- |
| Inner splice plate, M3 into heat-set inserts | Print splits | Strong, reversible, one piece reprints alone | Screws only from inside: assemble the unit on the bench | **Use for print splits** |
| Lap + locating pins + M3 to hardpoint | Service units | Repeatable location despite print error; outside access | Visible screw heads unless hidden | **Use for units**, heads in shadow gaps, under the arm and in the socket rim |
| Magnets + hinge tabs | Doors, hatches | Tool-free, fast | Can pop open in a fall | **Use for doors**, plus an optional M3 lock screw for test runs |
| Printed snap clips | Abdomen bands | No hardware | PETG fatigues, breaks in a fall | Coupon C5 first; fallback is M3 |
| Dovetail / sliding key | Print splits | Hidden, no hardware | Tight tolerances on curved parts, hard to reprint one side | Coupon C5 as an alternative |
| Glue (CA, epoxy) | | Simple | Not swappable, one crack means the whole unit | **Rejected** |
| Quarter-turn fasteners | Doors | Positive lock | Extra parts, bulk | Later option if magnets fail C6 |

## 4. Mounting and assembly

**Hardpoints (`../concept/generator/mounts.json`, orange in `SHELL_MFG_MOUNTS.png`):** 20 printed PETG brackets on M5 T-nuts,
each carrying M3 heat-set inserts. Panels carry only clearance holes and pins, so a reprinted panel needs no inserts. The panel boss
is designed to tear out before the bracket does, so a crash breaks the cheap part.

| ID | On | Qty | Carries | Gap to shell, mm (with the fix) |
| --- | --- | --- | --- | --- |
| H1 | Upper box top front rail, front face | 2 | U1 top edge | 33 |
| H2 | Upper box top rear rail, rear face | 2 | U2 top edge, D3 catch | 21 |
| H3 | Step shared rail, front and rear faces, between the gussets (Y ±35) | 4 | U1/U2 bottom, U3/U4 top, D1 frame | 43 front, 22 rear |
| H4 | Lower box bottom rails, front and rear faces | 4 | U3/U4 bottom, D2 frame | 15 front, 19 rear |
| H5 | Lower box corner posts, outer side faces | 4 | U3/U4 sides, V1 grilles | 25 to 32 |
| H6 | Lower box bottom side rails, outer side faces (under-face left free for the hip interface) | 4 | Abdomen band carrier | to check |
| C | Pitch RS03, 8 unused outboard M4 holes per side | 2 | Collar halves; U1/U2 socket rims lap on (location only) | 0 |

The gusset plates are not used as mounts, so they carry only frame load. The arm thread needs to keep these slot positions free
(**unresolved**: compute shelf, PDB and harness clips may already use some).

**Fasteners (estimate):** 40 × M5×20 + T-nuts for brackets (frame gate review: M5×16 left ~2 mm of thread through a 12 mm PETG foot); about 40 × M3 heat-set inserts (M3×5.7) with M3×8 button heads and
nylon washers; 8 × 6×3 mm N52 magnets per door; the existing collar M4×10 button heads.

**Assembly order:** frame and actuators in, harness dressed → hardpoints → collars (halves) → U3/U4 lower units (each assembled on
the bench first) → U1/U2 upper units → D1/D2 doors, D3 hatches, V1 grilles → abdomen carrier and bands. Every unit comes off
with the arms fitted, because the collar is split and the U1/U2 seam passes through the shoulder socket.

## 5. Access doors

| Door | Opens to | Retention |
| --- | --- | --- |
| D1 compute door, upper back, 120 wide, torso Y 374 to 492 (top raised 12 mm for the fans) | Pi 5, CAN HAT, Jetson Orin Nano rear ports (Y 376–445), two 40 mm exhaust fans (Y 448–488), pitch RS03 rear faces | 2 hinge tabs at the bottom, 4 magnets at the top, optional M3 lock |
| D2 battery door, lower back, |X| ≤ 60, torso Y 175–352 | 13S2P 21700 pack (95 × 162 × 79 at |X| ≤ 47.5, Y 183–345), pulled ~90 mm straight back on printed tray rails; XT90S and fuse at its top rear corner | Bottom hinge on H4, magnets onto a spine rail on the H3 rear bracket (Y 355–372), M3 lock screw for runs |
| Front power board (frame part) | Bus/BMS board in front of the pack, intake fan at Y 250–290 behind a U3 grille | Reached by removing U3 |
| D3 shoulder hatches, top of each shoulder | Pitch RS03 connectors, harness entry to the arm | Magnets, lip under U2 |
| V1 side grilles | Lower-box sides, side harness runs | M3 to H5 |
| Abdomen bands | Waist yaw RS03 and its harness | Carrier clips or M3 |
| Whole U1 unit | Front of everything, 4 screws | Hardpoints |

The battery lives in the pelvis (`../../../docs/torso-design-handoff.md`), so its door belongs to the pelvis shell.

## 6. Cooling (estimate)

Heat in the chest: two pitch RS03s (assume 15 W each on average when holding), Pi 5 (~8 W), Jetson Orin Nano (7 to 25 W by power mode), power board, BMS and converters (~5 to 10 W), pack (a few W at 40 A): **about 60 to 75 W**. Frame release 2477a12 adds two 40 mm exhaust fans behind D1 and one intake fan low at the front.

Chimney flow from low intakes to high exhausts, 0.3 m apart, 15 K rise: draft speed about 0.33 m/s, so natural convection needs
about **75 cm² net free area** at each end (about 150 cm² gross with a 50% open grille).

- **Intake (low):** V1 side grilles, 2 × ~130 cm² gross, louvres angled down; abdomen band gaps.
- **Exhaust (high):** the two 40 mm fans blow out through D1's louvres (louvre free area at least the fans' 2 × 12.5 cm²; fan guard 6 mm max); neck ring gap.
- **Front intake fan:** needs a front grille. Frame spec 6ba6c61 puts it at torso Y 250–295, X ±20, fully inside U3, which gets a front grille (≤ 6 mm slots) over it.
- **Variants:** D1 plain, louvred, or with a 40 × 10 mm fan (each ~2.4 L/s; two fans move ~4.8 L/s, enough for 70 W at a 12 K rise). V1 lattice
  or open mesh. The swap interface stays fixed.
- Slots no wider than 6 mm (fingers, cable snags). The shoulder RS03s also shed heat through their exposed output faces.
- **Unresolved:** real RS03 losses at the duty cycle. Log RS03 temperatures with and without the shell in the first powered run.

Crush bumpers (TPU 95A) under the shoulder socket rims and on the lower back corners give back some of the crush distance lost in
section 1, as another swappable part.

## 7. Material and print settings (to confirm with coupons and a slice)

- **Material: PETG** (Bambu PETG HF) for panels, doors and brackets. It is tough in a fall, and holds shape near warm actuators.
  PLA softens around 60 °C and is brittle. ASA is a later option for UV and heat. TPU 95A for bumpers.
- 0.4 mm High Flow nozzle (fitted), 0.2 mm layers, 6 walls (solid 2.4 mm), textured PEI plate. Bosses at 100%.
- Orientation from the script (`SHELL_MFG_PLATES.png`): large panels stand on an edge so the visible face is a wall surface, with
  tree supports only on the inside. Seam position: rear, aligned to the shadow-gap lines. Brim on the tall standing pieces.
- A full 3MF and an exact-settings checklist come with each print release, after gate 2.

## 8. Pre-print validation ladder (Joey's ask: simulate and test before printing)

Each stage is cheaper than the next. Nothing goes to a full panel until the earlier stages pass.

| Stage | Where | What | Pass |
| --- | --- | --- | --- |
| 0. Split check | here, **done** | Pieces fit 250 mm, orientation, mass, time; frame clearance | All fit; ≥ 5 mm to the frame |
| 1. CAD | SolidWorks, Joey-PC | Real panels with laps, bosses, splice plates, pins. Interference detection against the frame and the arm sweep. Draft and overhang analysis | 0 interferences; arm sweep ≥ 3 mm (arm-thread envelope) |
| 2. FEA | SolidWorks Simulation | Boss tear-out vs bracket pull-out (fuse order); splice plate bending; door retention; Drop Test study of U1 and U2 at the trip-fall speed (~3.8 m/s) | Panel fails before the bracket; doors stay shut; no frame contact |
| 3. Slice | Bambu Studio, Joey-PC | Real time, mass and support; seam placement; warp risk | Matches stage 0 within 25% |
| 4. Coupons | P2S, ~1 h each | C1–C8 below | Criteria below |
| 5. Seam strips | P2S, draft settings | 30 mm strips across each seam and hardpoint, plus one full panel in draft (0.28 mm, 2 walls) fitted to the frame | Pins engage, shadow gap 1 ± 0.4 mm, no forcing |
| 6. Gate-2 review | Reviewer CLIs | Packet: STEP, STL, renders, this README, FEA and coupon results | Findings reconciled |
| 7. Print | P2S | Final panels, one unit at a time | |

**Coupons:**

| ID | Test | Pass |
| --- | --- | --- |
| C1 | Tolerance comb for laps and pins, 0.10 to 0.40 mm in 0.05 steps | Pick the slip fit; reuse everywhere |
| C2 | M3 heat-set insert pull-out and torque-out in the bracket boss (3 boss sizes), with a luggage scale or water bucket | > 150 N pull |
| C3 | Panel boss tear-out | Lower than C2 (the fuse order) |
| C4 | Splice joint across two curved 80 × 80 wall coupons: bend and drop | No crack at the splice before the wall |
| C5 | Dovetail key and snap clip coupons vs the splice | Decide the abdomen band clip and any hidden joins |
| C6 | Door on a frame coupon, dropped 0.75 m onto plywood, 10 times | Door stays shut; magnets stay seated |
| C7 | Wall coupons at 2.0, 2.4 and 3.0 mm, 1 kg drop weight | Pick the wall thickness |
| C8 | Grille sample: free area and a finger probe | ≥ 50% open, slots ≤ 6 mm |
| C9 | TPU pad crush: 60 × 60 × 30 blocks at 3 infills, weight drop or slow press | Absorbs the per-pad share of the back-fall energy within 25 mm |

## 9. Open items

**Arm-thread tip-over recheck (main 022da44):** with only 10 mm of stop the frame yields (side 400 MPa, back 584, limit 145). It holds
with about 30 mm of stop at the side and 40 mm at the back. The side gets there because the arm stands over 100 mm proud and crushes.
The back does not: **the shell needs a back crush pad (or hip guard) giving about 30 mm of crush beyond the shell.** First pass modelled (`SHELL_MFG_SPINEPACK.png`): six TPU 95A pads standing 30 mm
proud of the back, flanking D1 and D2 so the doors open between them, bolted through the shell to H2/H3/H4 rear on standoff posts so a
fall loads the brackets, not the shell wall. About 258 g at 2 walls + ~10% gyroid (coupon C9 tunes it); EVA foam in a thin TPU
skin is a ~60 g alternative. Extents in the torso frame: upper pads |X| 68–147, Y 368–481; lower pads |X| 64–116, Y 166–295 (inner edges 6 mm clear of the D1/D2 openings, frame gate review);
the spine bridges are gone (the battery door now runs up to Y 352); all Z −77 to −124. Styling comes in SolidWorks.
**Frame gate review (main 6ab5703):** pad standoffs need a metal load path, clamped through to the rail (aluminium standoff on an M5
through the PETG bracket into the T-nut), not just the PETG bracket. The back needs **40 mm** of stop; the pads give 30, so either 40 mm
pads or pad plus standoff crush. No back-fall survival claim until coupon C9 gives a force-travel curve. Battery locked 2026-11-06 06:13Z: custom 13S2P pack inside
the rev A lower box (25 mm behind the frame rear face), so the pads stay as the rear crush element. D2 must not be a crush path into the pack. One-face
loading is harsher than a real fall, but it is the sizing target.
Hardpoints checked in CAD: H1, H2, H4, H5, C and H3 free. **H3 rear bracket stays below Y 374 (torso frame)**: the Pi sits above it and its
rear ports need plug room in the gap behind D1. H6 needs a check on the robot (waist adapter not in CAD).

1. ~~Joey: OK the back clearance fix~~ approved 2026-11-06.
2. Arm thread: keep hardpoint slots free (section 4), and update the tip-over crush distance (section 1).
3. Model the chest shield over the sternum seam; seat the D1 louvres.
4. SolidWorks panel modelling on Joey-PC (stage 1), then FEA and slicing.
5. Real RS03 heat at duty cycle (powered run with the shell).

## Reproduce

In `../concept/generator/`: `npm install three@0.170.0`, serve on 8765 (`python3 -m http.server 8765`), then
`node dump.js "" out/grids.json`, `python3 split_check.py`, `python3 frame_clearance.py`,
renders with `node shoot.js` (`split:` prefix for the `split.html` modes: assembled, explode, mounts, air, plates; `DPR=2`) and
`python3 compose_mfg.py`. Needs `pip install trimesh manifold3d shapely scipy`.
