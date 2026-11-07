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
| H2 | Upper box top rear rail, rear face, Y ±85 (clear of D1, |Y| ≤ 60) | 2 | U2 top edge, D3 catch, upper spine-pad posts | 15.3 (SolidWorks) |
| H3 | Step shared rail, front and rear faces, between the gussets (Y ±35) | 4 | U1/U2 bottom, U3/U4 top, D1 frame | 43 front (54.7 along the panel normal at Y 35, SolidWorks pilot), 22 rear |
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
| Front power board (frame part) | Bus/BMS board in front of the pack; two 40 mm intake fans (X ±20, Z 70–80) behind two U3 grilles: Y 264–304 (BMS FETs, E-stop) and Y 193–233 (logic-board DC-DC, brake resistor on the −X bottom side rail, Y 155–180) | Reached by removing U3 |
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
- **Front intake fans (frame 733b9d0):** two 40 mm fans, X ±20, fan bodies Z 70–80 behind U3: upper Y 264–304 (frame d01106a, dropped 8 mm so the U1/U3 seam lap at Y 315 stays intact), lower Y 193–233. U3 gets two front grilles (≤ 6 mm slots, ~40 × 40 open each).
- **Variants:** D1 plain, louvred, or with a 40 × 10 mm fan (each ~2.4 L/s; two fans move ~4.8 L/s, enough for 70 W at a 12 K rise). V1 lattice
  or open mesh. The swap interface stays fixed.
- Slots no wider than 6 mm (fingers, cable snags). The shoulder RS03s also shed heat through their exposed output faces.
- **Unresolved:** real RS03 losses at the duty cycle. Log RS03 temperatures with and without the shell in the first powered run.

Crush bumpers (TPU 95A) under the shoulder socket rims and on the lower back corners give back some of the crush distance lost in
section 1, as another swappable part.

**Frame gate review 2 (8a15651) carried into the shell:**
- D2 magnets sit inside |X| ≤ 52: the spine rail lip was narrowed to clear the rear gussets and their screw heads.
- The lower box has no path up into the upper bay (shelf and shared rails). It exhausts sideways through the **V1 side grilles**.
  The only exit from the board zone is a ~12.5 mm window each side at torso Z 37.5–50, Y 180–348 (the pack tray rails block
  Z −50 to 37.5; the cable band shares Z 42–50). So each V1 gets **up-angled exhaust louvres toward its front edge (torso Z
  ~35–55)**, and the rest of V1 stays intake. The two front fans pressurise the lower box. Pack heat (battery thread) is ~20 W
  steady and 36 W worst case; 133 W at 40 A is only a 10 s burst. Log pack and air temperatures in the first powered run.
- Spine-pad standoffs start behind torso Z −82, so they clear the H2 brackets and the gusset screw heads (Z to −77.5).
  H2 moved to |X| 85 (0bfa66a), so the old overlap at |X| 45–75 is gone; the pads only have to clear the brackets' new footprint.
- The D3 shoulder-hatch cut keeps ≥ 1 mm from the H2 boss (Z 1288–1299, |Y| 77–93).

## 7. Material and print settings (to confirm with coupons and a slice)

- **Material: PETG** (Bambu PETG HF) for panels, doors and brackets. It is tough in a fall, and holds shape near warm actuators.
  PLA softens around 60 °C and is brittle. ASA is a later option for UV and heat. TPU 95A for bumpers.
- 0.4 mm High Flow nozzle (fitted), 0.2 mm layers, 6 walls (solid 2.4 mm), Cool Plate SuperTack, no glue. Bosses at 100%.
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
| C10 | Screw cap Ø7.9 in Ø8.2 × 4.2 counterbore, clearance steps 0.10–0.20 mm | Cap stays in by hand, pries out with a fingernail |
| C11 | Bracket foot with the 5.8 × 1.6 slot key on a real 2020 rail + T-nut, M5 torqued | Key seats, bracket does not rotate |
| C12 | D2 magnet pair in shear (door block vs U2 pad), plus the M2.5 plastite lock | Door holds shut over C6 drops with locks out; record the pull force |
| C13 | M2.5 × 6 plastite into the Ø2.2 × 5 pilot with 1 mm skin, 5 cycles | No strip, no bulge on the outer face |
| C14 | Lower back panel printed tilted 45° (5 h) vs split face down (9 h) | Accept tilt only if the face-up stepping is invisible at arm length |

## 8b. SolidWorks pilot, upper front left (2026-11-06)

First real panel built on Joey-PC from the section CSVs (native files in the CAD folder, not in git). Loft of 40 sections clean.
**2.4 mm would not thicken** (2.3 did): the socket lip had a ~0.4 mm convex radius (smoothstep over 7 mm). Fixed in the generator:
parabolic lip over 14 mm (~5 mm radius, `SHELL_SOCKET_LIP.png`), cavity floor r78 unchanged, the wall only moves away from the arm.
The section export missed the sternum groove (no samples between Y 0 and 32); now exported from an 8x denser grid, resampled by
arc length, 80 sections for the upper panels (`sections/`, `sections_report.json` lists the tightest radius per panel).
Pilot mass 168 g (2.3 mm wall), box 125 × 154 × 228. Bracket standoffs: H1 27.5 mm, H3 front 48.7 mm (boss moved to Y 35 so it
clears the sternum splice holes). Fixes for the next build: bottom shadow-gap chamfer 0.6 mm, not 1 (it left a 0.2 mm feather
edge); the H1 screw goes through the open chest, so screws move to blind bosses from inside or under the shield; STL to be checked
in the slicer (249 non-manifold edges after merge; the STEP is one solid).

**Rev 2 (pilot on the new sections):** 2.4 mm thickened only because SolidWorks patched two spots: wall down to 0.57 mm at
(45.7, 135.5, 1297.9) and 1.63 mm at (48.7, 135.5, 1174.4), the top and bottom of the socket. Otherwise clean: 175.1 g, box
124.2 × 150.5 × 228.0, sternum groove within 0.001 mm, watertight STL with 0 non-manifold edges. Bambu Studio: sternum-down
10 h 10 m, 160 g + 93 g support, none on the outer face (socket-down 7 h, but support on the visible socket wall). The support
estimate in `split_check.py` (a few g) is far too low for curved panels; recalibrate against slices before trusting print times.
Cause: at the socket's top and bottom the lip rose straight into the chest's front corner rounding, and the old y-map left a
crease where the original side crossed the floor plane. **Socket rev 3** (`SHELL_SOCKET_REV3.png`): the cavity is a cone of
revolution about the pitch axis (flat floor at the RS03 face out to r78, 6 mm filleted foot, slope 1.2) and the side is the
smooth minimum of the original surface and that cone (22 mm quadratic blend). The blend only moves the wall inward (floor dips
at most 5.5 mm along the old crease line, at r > 60, clear of the RS03 body r 49.75), so arm clearance is unchanged and frame
clearance is unchanged (7.8 mm minimum at the gussets). Inward offset folds at 2.4 mm (`thicken_check.py`): 0 / 0 (rev 2:
908 front, 1743 back). Tightest convex radius in the socket sections ≥ 5.0 mm (`curv_check.py` on the CSVs, `curv_grid.py` on
the dense grid); the shoulder top cap at ~3.3 mm is the tightest spot left and thickened fine in both pilots. Upper panels
now ~170 g each from the split check. `?rim=14` in the generator restores rev 2.

**Rev 3 pilot (SolidWorks):** 2.4 mm thickens with no patching (6 faces vs 25); wall 2.399–2.401 mm over 6,326 points,
including both old thin spots, the socket and the top cap. 170.0 g, box 124.2 × 144.8 × 228.0, H1 27.4 mm, H3 48.5 mm; STL watertight,
0 non-manifold edges (the first export from the build session lacked the inner face: re-check STLs exported straight after a build).
Slices (from G-code, `pilot/`): sternum down 11 h 04 m, 157 g + 106 g support, outer face clean; sternum up 7 h 13 m, 30 g support but
marks on up to ~137 cm² of the socket wall; neck down 7 h 20 m, 36 g, marks at the top. STEP reimport reads 0.3% high
(unexplained): print from the STL.
**Decision (Joey, 2026-11-06 09:48Z): chest panels print sternum-down**, keeping the outer face clean and accepting ~11 h and ~106 g of support each.

**All 8 chest panels built in SolidWorks (2026-11-06, renders in `pilot/`):** every one thickens clean at 2.4 mm (min wall 2.399 mm),
0 non-manifold edges, no support on the outer face in the default (split face on the plate) orientation.

| Panel | Mass | Box (mm) | Default slice | Support |
| --- | --- | --- | --- | --- |
| upper_front L/R | 170 g | 124 × 145 × 228 | 11 h 05 m | 106 g |
| upper_back L/R (D2 notch, H2 at Y ±85) | 164 g | 98 × 146 × 228 | 11 h 30 m | 117 g |
| lower_front L/R (2 grille cuts) | 94 g | 107 × 142 × 175 | 8 h 40 m | 85 g |
| lower_back L/R (D2 cut, 1.6 × 6 return flange) | 77 g | 96 × 141 × 175 | 9 h 13 m | 97 g (tilted 45°: 5 h 20 m, 42 g, needs a finish test) |

Panel set ≈ 1.01 kg and ~81 h of printing at the defaults. Screw caps: 22 per robot (Ø7.9 × 4.2, solid 2.35 mm roof) in Ø8.2 × 4.2
counterbores over Ø12 inner pads. H5 bosses sit on the panel normal (37° front, 22° back): the brackets take the angle.

**Doors, grilles and hatches built (2026-11-06 evening, `pilot/`):** all clean 2.4 mm, 0 non-manifold, no support on the outer face.

| Part | Mass | Slice | Notes |
| --- | --- | --- | --- |
| D1 compute door | 39 g | outer up, 4 h 25 m, 32 g support | slotted hinge tabs at Y ±52 on H3 rear (pull out, then swing); 4 magnets Y ±50.5; M3 lock Y 0 Z 1240; 2 louvre patches Y ±25, Z 1258–1298, 6 slots 5.5 × 38 tipped 30° up, 12.5 cm² each |
| D2 battery door | 58 g | outer up, 8 h 06 m, 55 g support | hinge tabs Y ±52 on H4 rear; retention as built below; ~40 mm to the pack |
| Intake grilles (2) | 6 g | outer up, ~1 h | 58% open, 6 slots ≤ 6 mm; held by 4 M2.5 × 6 plastite per grille into blind pilots in U3 (also ties the U3 halves); fit before U3 goes on |
| D3 shoulder hatches | 11 g | upright, 52 m | first pass: still needs the 0.3 mm gap, lip under U2, magnet pockets and the matching U2 cut; 1.75 mm to the H2 boss |

Upper backs now carry the D1 opening and flange (~145 g each). D1's top flange touches the Z 1310 splice pad at Y 6–18: the splice
plate must clear it. Not yet checked: D1 against the fans, Pi and Jetson behind it.

**Shoulder hatches, socket hole and collars (evening 2026-11-06):**
- D3 hatches are finished: opening + 0.3 mm gap + 1.6 × 6 flange frame on U2, front tongue, and magnet-to-magnet retention (10 × 3
  magnets in Ø10.2 × 3.1 pockets on hatch pads and two U2 flange tabs, 0.5 mm air). The hatch is 11.4 g and prints in 1 h 06 m.
- Socket hole r 53.5 in the upper panels (the M4 button heads reach r 52.85). Collar: ID 86 / OD 114, split at X 0, stepped section
  (flange Y 132.5–134.5 out to r 53.2, outer ring Y 130.6–132.6 out to r 57), so the panel floor laps r 53.5–57 with ≥ 0.27 mm clearance.
  Holes are clocked at 22.5° + 45k. One part, print 4: 5.9 g, 18 min.
- Upper panels with all cuts: front 147 g / 12 h 03 m, back 115 g / 11 h 02 m (left; right matches).
- **Check on a real RS03 before torquing the collar screws:** the vendor drawing gives the 8 housing M4 holes 8 mm deep on a 98 mm
  BC (r 49), but in the CAD model their Ø3.3 tap drills break the r 50.25 housing wall. The model is probably simplified there.

**Hardpoint brackets (16, PETG, ~151 g total, renders in `pilot/`):** H1 front hook, H2 rear hook, H3 front (Ø3.7 pin + M3 insert),
H3 rear (D1 hinge arm, door top-stop lip Y 0–52, Z 1165–1182), H4 front, H4 rear (D2 hinge arm), H5 front 37° and back 22°, each L/R.
Each prints in 22–41 min. M5 into a T-nut, offset from the post so a driver reaches it, with a 5.8 × 1.6 slot key against rotation
(H4 front has no key: it is shortened to clear the splice boss). Door hinges pivot on **M3 × 12** screws into inserts in the
arms. Clash checks: no frame clash (the slot keys sit 1 mm into the slot by design, so check on a real rail); gussets clear by position
(H3 stops at |Y| 54, plates start at 55, nearest screw head 57.25; H5 tops out at Z 1093, gussets start at 1095). Shell touches only at
the boss faces, 0.3 mm at each hinge.

**D2 retention, as built:** magnets work vertically. Pads on U2 at Y ±30 (Z 1158–1163) hold magnets facing down onto door blocks
(Z 1150–1157.7) facing up, 0.5 mm air. The door slides off them sideways as it opens, so hold is shear (~1/3 of axial): **test on the
real door.** Locks: 2 × M2.5 × 8 plastite at Y ±53 (pads start at |Y| 50, 2.5 mm clear of the pack's slide path), Z 1153 into U2 side-flange pads (caps: 23 per robot). Nothing on U2 sits below
Z 1158 in the pack column. Rule agreed with the frame thread: the XT90S/fuse stays inside |X| ≤ 45, torso Z ≥ −68, so nothing on the pack reaches the door
(22.8 mm from the door blocks to the pack sense board).

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
