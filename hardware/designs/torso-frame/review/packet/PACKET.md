# Gate review packet: internal torso frame rev A

2026-11-06. Thread "Finalize the internal torso frame". Review gate: **before any aluminium is cut** (project rule: adversarial
review at layout freeze, before printing/promotion, before load). This is the layout freeze for the frame.

**Reviewer instructions.** Open the files below, not only this summary. Attack the spec: wrong numbers, missing load paths,
parts that cannot be assembled in the stated order, packaging that does not fit, fasteners that cannot be reached, anything that
would make Joey cut aluminium twice. Give findings as a numbered list, each with: severity (blocker / must fix before cutting / can
wait), the file and line or dimension it refers to, your evidence or arithmetic, and the fix you propose. End with a clear GO or
NO-GO for cutting the extrusion and gussets. Do not restate the spec.

## Files (all copied into this packet)

| File | What |
| --- | --- |
| `data/torso-frame-README.md` | The spec: geometry, cut list, joints, fasteners, shopping list, parts, interfaces, packaging, changes, open items |
| `scripts/frame.py` | Parametric source of every part position (torso coords, mm) |
| `data/parts.json` | Generated part list with boxes/cylinders/polygons, explode vectors, masses |
| `data/bom.json`, `data/cutlist.md` | Generated BOM and cut list |
| `views/FRAME_*.png` | Exploded, fastener detail, assembled, dimensions, inside, packaging sections, hardpoints, changes |
| `data/torso-stepped-README.md` | Arm thread: step geometry, FEA (peak use, tip-over), gusset plate, mid-tier options, crush distance |
| `data/gusset_plate_left.dxf` (STEP files stay in the repo under hardware/designs/torso-stepped/) | Gusset outline (R8 everywhere; the spec raises the X 113 / Y 335 corner to R15) and the FEA frame body |
| `data/body-shell-manufacturing-README.md` | Shell side: hardpoints H1-H6, doors, spine pads, cooling |
| `data/mounts.json` | Hardpoint coordinates (robot frame) |
| `data/torso-2020-extrusion.md`, `data/torso-actuator-brackets.md`, `data/torso-design-handoff.md`, `data/master-bom.csv` | The built frame and its brackets (pre-rev A; some numbers superseded) |

## Checks done

- **Script check:** all part positions generated from one parameter set; cut list and fastener counts derived from it.
- **Recorded CAD check (arm thread, master torso 022da44, 2026-11-06 04:46Z):** Jetson, fans, Pi, IMU, PDB envelope and shelf clear the real RS03 and pitch bracket; four corner brackets moved (now 2020 type) after a 3 mm overlap was found; shelf plate is X ±55, Y 354.6 to 372.6; pitch bracket envelope X 78.4 to 113, Y 353.6 to 495, Z ±50, wing ±69 at Y 400 to 450.
- **FEA (arm thread, torso-stepped):** peak use and three tip-over cases, bonded joints, 6063-T5; 3 mm 6061 gussets required for the tip-over; the frame needs ~30 mm crush at the side and ~40 mm at the back.
- **Cross-thread agreement:** gusset radii, H6 move, H3 rear below Y 374, spine pad geometry, D1 door extent, intake fan position.

## Not verified (say so if you rely on these)

- The built frame's real rail count and the waist adapter bolt pattern (photo requested from Joey).
- Pitch bracket to post fastening: 8 × M5 is an assumption; the built bracket's pattern decides.
- RS03 connector flat (A high rear or B low front) per motor: unresolved; harness exit depends on it.
- 2020 mass 0.5 g/mm, T-nut friction, bracket stiffness: assumptions from the torso-stepped README.
- PDB size and placement (board not designed). Main battery location (card open with Joey; pelvis is the baseline).
- Printed part envelopes (hardpoint brackets 30 × 20 × 12, Jetson shelf bracket) are placeholders, not designed parts.
- Fastener lengths: M5 × 10 into a 2020 T-nut through a 4 mm bracket leg; M5 × 16 through 12 mm of PETG; check engagement.

## Pointed questions

1. The step joint: the lower post butts into the shared rail underside and the upper post sits on its top, 38 mm outboard. Is one inner-X corner bracket per post end plus the two 3 mm gussets enough, or do the post Z faces need brackets too (the optional +8)? Consider the side tip-over with T-nut slip.
2. The gussets bolt into the T-slots of the posts and the shared rail with 9 × M5 each, 860 N per bolt in the tip-over. Will the T-nuts slip or the 3 mm plate bear out before the frame yields? Is R15 at the X 113 / Y 335 corner enough?
3. Assembly order: can the shared rail's T-nuts for the shelf rails, step side rails, gussets and H3 brackets all be inserted and reached with the posts in place? Which slots are consumed by which parts, and where do the drop-in T-nuts go?
4. Packaging: the upper bay is 156.8 × 100 × 101.4 mm with the Pi stack (21 mm) and the Jetson dev kit (35 mm + fan) on two shelves, 30 mm left under the top rails for two 40 mm fans. Is this serviceable through a 120 × 132 mm rear door, including the Jetson's rear ports and the Pi's ports 1.8 mm proud of the frame face?
5. The 2020 brackets (20 mm legs) at the four top inner-X corners replace 2028s to clear the pitch bracket wing. Do they weaken the top corners meaningfully under the peak-use arm load?
6. Anything in the shopping list that is wrong or missing for a build from the existing frame (one more 4-pack of 2020, 100 T-nuts, 100 M5 × 10, 25 M5 × 12, 25 M5 × 16, 3 mm 6061 sheet, 4 × 2020 brackets)?
7. The waist RS03 is 57 mm tall in a 40 mm bay and the main battery is in the pelvis: does anything in this frame spec make the hip-stage work harder than it needs to be?


Hashes: SHA256SUMS.txt in this folder. Scripts: `python3 scripts/frame.py` regenerates data/parts.json, bom.json and cutlist.md from the parameters (run it in a scratch copy, not here).
