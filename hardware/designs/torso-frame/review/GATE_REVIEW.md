# Gate review, internal torso frame rev A (2026-11-06)

Packet: `review/packet/` at main 18fda31 (spec before the battery rework). Reviewers ran read-only on Joey-PC from inside the
packet folder, per `hardware/docs/review-recipe.md`. Hashes checked before and after each run: nothing in the packet changed.
Raw reviews and logs: `review/packet/reviews/`.

| Reviewer | Tool | Exit | Elapsed | Verdict |
| --- | --- | --- | --- | --- |
| Sol | Codex `gpt-6.1-sol` | 0 | 553 s (9.2 min) | NO-GO extrusion, NO-GO gussets |
| Astra | Codex `gpt-6-astra` | 0 | 401 s (6.7 min) | NO-GO, NO-GO |
| Grok | cursor-agent `grok-4.7-high-fast` | 0 | 750 s (12.5 min), after a 30 s failed first attempt | NO-GO, NO-GO |
| Wonder | Muse `muse-wonder-1.0` | 0 | 308 s (5.1 min) | GO on the new stock lengths; NO-GO on cutting the built posts and the gussets |

Main run 04:59:44Z to 05:08:57Z (all four in parallel). Grok's first attempt from Git Bash exited 0 in 30 s but every file and
shell call was rejected by the repo's `.cursor` pre-tool hook (a bash syntax error in `impeccable-before-edit.js`); it is kept as
`cursor-grok-attempt1-failed.md`. The rerun from PowerShell with the same flags is `cursor-grok.md`. Add that to the recipe.

**Scope note.** The packet predates Joey's battery, power-board and cooling requirements (05:0x), so none of the reviews cover
them. Findings about the pelvis battery and the in-torso option are superseded by the option D work (`../FRAME_OPTIONS_D.png`).

## Accepted and fixed in this revision (main, this commit)

| # | Finding | Raised by | How I checked | Fix |
| --- | --- | --- | --- | --- |
| 1 | The four lower-post step brackets were generated against the upper-post width: legs at X 65 to 93 instead of 27 to 55, 38 mm off the lower post | Sol 1, Astra 1 | `parts.json` br2/br8/br14/br20 before: leg X 65–93; `frame.py` BX() used `y < Y_STEP_LO` | `y <= Y_STEP_LO`; br2 now X 27–55 with its leg on the post face X 55 |
| 2 | T-nuts modelled 20 mm long along the slot: they overlap at the 15 mm gusset pitch and run past the rail end at X 113 | Sol 4, Astra 2, Grok 6 | `tnut()` set ±10 along the slot | 10 mm nuts (standard 2020) |
| 3 | Gusset mass 10 g per plate; it is about 40 g (4930 mm² net × 3 mm × 2.70) | Sol 13, Astra 13, Grok 11, Muse 11 | recomputed from the outline in `gusset_dxf.py`: 39.9 g | BOM 160 g for four |
| 4 | Step side rail 2028 brackets run 10 mm into the shelf rails (18 mm gap between X 75 and 93) | Astra 1 | br25 before: leg X 65–93 | 2020 brackets there; 8 × 2020 total, 24 × 2028 |
| 5 | M5 × 16 through the 12 mm PETG hardpoint brackets leaves about 2 mm of thread | Sol 10, Astra 6, Grok 11, Muse 6 | 16 − 12 − 1.5 slot wall | M5 × 20 |
| 6 | The Jetson envelope omitted its 10 mm cooler: 20 mm to the top rails, not 30 | Sol 6, Astra 8, Grok 4 | 475 − 455 | envelope Y 410–455 |
| 7 | The two exhaust fans at Y 448–488 blow 13 mm of their face into the rear top rail (Y 475–495) | Sol 6, Astra 8, Grok 4, Muse 7 | coordinates | moved to the D1 door plane behind the frame face, Y 430–470, Z −70 to −80, marked provisional (needs 12 mm of shell gap); the power-board rework redoes cooling anyway |
| 8 | No cutting file contains R15: `torso-stepped/gusset_plate_left.dxf` is R8 at every corner | Sol 3, Astra 2, Grok 1, Muse 1 | read the DXF arcs: centre (105, 337.695) R8 | `gusset_plate_left.dxf` generated from the spec geometry (`generator/gusset_dxf.py`): true arcs, R15 at (113, 335), nine Ø5.5 holes; the stepped README now points at it |
| 9 | Envelopes did not match their labels: PDB 24 deep (said 28), third board 90 × 90 (said 90 × 60), intake fan 45 tall | Astra 12 | `frame.py` | corrected |
| 10 | Lower free height is 178.6, not 198.6; waist deficit is 23 mm with the 6 mm adapter, not 17 | Sol 13, Astra 5 and 13, Muse 9 | 353.62 − 175; 57 + 6 − 40 | README |
| 11 | D1 door quoted two ways (Y 360–480 here, 360–492 in the shell plan) | Sol 7, Astra 9, Grok 5, Muse 7 | both files | README uses 360–492 |
| 12 | Shopping list: M5 × 10 count is exact with no spares; nut types not assigned; "offcuts cannot yield the rails" was asserted without an inventory; the two spare 110 rails can be cut to the 100 shelf rails | Sol 11 and 12, Astra 11, Grok 10 and 11, Muse 3 and 12 | arithmetic | list rewritten: slide-in 100 + drop-in 50, spare M5 × 10, stock conditional on Joey's photo |

## Accepted, still open (needs Joey, the bench, or the battery decision)

| # | Finding | Raised by | Why it waits |
| --- | --- | --- | --- |
| A | Pitch-bracket screws assumed at Y 390 and 470 land on the step and top corner-bracket nuts in the same upper-post slot; M5 × 12 from X 78.4 stops 2.6 mm short of the post face | Sol 4 and 10, Astra 6, Grok 8, Muse 2 | **Resolved from the original STEP (arm thread, 06:09Z):** each bracket bolts through four slots in 4 mm ears at X 89 to 93, heads seated at X 89: step side rail (Y 363.69, Z 0), top side rail (Y 485, Z 0), front and rear upper posts (slot Y 416.6 to 432.8, Z ±60). Nothing at Y 390 or 470, so no clash with the corner-bracket nuts (step bracket nut ≈ Y 387, top bracket nut ≈ Y 465, both ≥ 16 mm from the slots). M5 × 12 from X 89 leaves 8 mm past the ear: fine. Bolt size (M5 or M6) still to be read off the built robot. Generator updated |
| B | Step joint capacity: side-fall shear runs along the gusset slots, so the joint is T-nut friction (~1 kN assumed) against 860 N per bolt, and the 5-bolt sharing is unexplained (9 holes, 3 per member). The "+8 brackets" option is the same friction joint | Sol 2, Astra 3, Grok 3, Muse 4 | agreed. Grok's fix is the right one: one positive end anchor per post through the clear rail face (above the lower post, below the upper post). Design it with the battery-driven frame |
| C | No slot-by-slot nut map or build order; slide-in nuts cannot reach the shared-rail top and bottom slots after the posts are on | Sol 5, Astra 7, Grok 6, Muse 5 | write it for the final frame; Grok's order (park nuts, posts and inner-X brackets, shelf and step rails, outer-slot nuts, gussets, H3) is the draft |
| D | Back-fall survival routes through TPU pads on PETG hardpoints with no force-travel curve; 40 mm of stop is needed and the pads give 30 | Sol 8, Astra 4, Grok 9, Muse 8 | shell thread (coupon C9, metal load path for the pad standoffs). Option D2/D3 packs behind the back change this case entirely |
| E | Pads at |X| ≥ 58 (upper) and ≥ 54 (lower) overlap the door openings |X| < 62 and < 58 by 4 mm per side | Sol 7, Astra 9 | sent to the shell thread |
| F | Built-rail inventory: the as-built list has 2200 mm and four 100 mm rails; the spec assumed six | Sol 12, Astra 11, Grok 10, Muse 3 | Joey's photo of the built frame |
| G | Pi ports 1.8 mm proud against an 8 to 17 mm shell gap: no straight plug fits with the door shut | Sol 7, Astra 9, Grok 5, Muse 7 | right-angle cables, or the deeper back in option D |
| H | Fastener schedule lacks Jetson shelf, fan, pad-standoff and adapter hardware | Sol 11, Astra 11, Muse 12 | after the battery decision |
| I | Shelf-end brackets would cut the shelf plate's corners if the plate reaches |Z| > 46 | Sol 5, Astra 1, Grok 7 | measure the plate's Z on the robot |

## Rejected or downgraded

| Finding | Raised by | Why |
| --- | --- | --- |
| Step side rails "sit inside the recorded pitch-bracket envelope" (blocker) | Grok 2 | The envelope X 78.4–113 is a bounding box that includes the upper post itself (X 93–113); the rail is the post's continuation downward in the same plane. Grok says as much ("may be a false clash"). Downgraded to a one-line check in the SolidWorks interference run, not a blocker |
| "Offcuts can supply the rails, so the 4-pack may not be needed" | Sol 12, Astra 11 | Grok's packing (a 340 cannot share a 500 stick) shows the leftovers are one full stick plus short offcuts, and 1024 mm does not come from 500 + 300. Buy the pack. The inventory still has to be photographed |
| Four 2020 top brackets need their own strength case | Sol 9, Astra 10 | Peak use is 7 MPa at the 99th percentile; Muse and Grok agree it is not the fuse. Can wait: inspect after a fall |
| Waist RS03 intrusion "competes with the pelvis battery" | Sol 14, Astra 5, Muse 9 | Superseded: the pelvis battery baseline is gone (option D) |
| In-torso battery option conflicts with the PDB / pocketed adapter | Muse 9, Grok Q7 | Superseded by option D |

## My own errors

The step bracket bug (finding 1) was mine and sat in every exploded view; the balloons hid it because the bracket was drawn against
a rail that does exist. The 20 mm T-nut and the 10 g gusset were placeholder numbers that I never replaced. The spec claimed R15
while pointing the laser job at an R8 file. The "cannot yield the rails" line in the shopping list was asserted without an inventory.

## Next

The frame is being repackaged around Joey's battery choice (`../FRAME_OPTIONS_D.png`). The repackaged spec gets a new packet and a
new round; items A, B, C and F above go into that packet as explicit questions.
