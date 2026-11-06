**NO-GO.** Do not cut the four 340 mm posts, and do not laser the gusset DXF in this packet.

The extrusion lengths are internally consistent. The irreversible cuts are not. The plates in `data/gusset_plate_left.dxf` are not the plates the spec says to cut.

## Findings

**1. Blocker. Gusset file is R8. The spec you would cut to is R15.**  
`data/gusset_plate_left.dxf` fillet at the X 113 / Y 335 corner is an arc, center (105, 337.695), radius 8, sweep −37.23° to 0°. That is an R8 fillet of the corner (113, 335): the diagonal from (75, 285) to (113, 335) is 62.8 mm at 37.23° to vertical, and 113−8 = 105. Holes match the spec (Ø5.5 at the nine listed centers). No R15 entity exists. `data/torso-frame-README.md` line 117 and `data/torso-stepped-README.md` lines 83–85 require R15 because the side-fall peak, 297 MPa, sits on that corner. 297 MPa is above 6061-T6 yield (~276 MPa). The 99th percentile they say to use is 100 MPa, under the 145 MPa they assume for 6063-T5. R15 was not re-solved.  
Smallest fix: edit that one fillet to R15 (center X = 98) and replace the DXF before any laser job. R15 fits the 62.8 mm diagonal and the 90 mm vertical edge, and the nearest hole (104, 363.62) is ~30 mm away, so the radius does not eat a hole.

**2. Blocker. The step side rails sit inside the recorded pitch-bracket envelope, and the drawings hide it.**  
Recorded envelope (`PACKET.md` checks, `torso-frame-README.md` line 121): X 78.4 to 113, Y 353.6 to 495, Z ±50. Step side rail from `scripts/frame.py`: X 93 to 113, Y 353.62 to 373.62, Z −50 to 50. That rail is entirely inside the box. The four step-side 2028s are too: corner (93, 363.62, 50), one leg X 65–93 at Z 46–50, the other X 89–93 at Z 22–50.  
The generator draws the bracket only from Y 373.62 (`frame.py` line 199). The sheets are that model, so they cannot show the bottom 20 mm of the CAD box. The CAD check cleared the shelf, Pi, Jetson, fans, IMU, and PDB. It did not clear these rails. Bracket mass is 120 g against a solid box of that envelope (~620 g of PETG), so the box is mostly empty and this may be a false clash. It is still the reason the posts get cut.  
Smallest fix: one interference check of the real pitch-bracket solid against the moved step side rails and those four 2028s. No post cuts until it is clean.

**3. Major. Side-fall load on the gusset is along the slots. The bonded FEA does not apply.**  
`torso-stepped-README.md` lines 47–49 and 83–85: joints in the model are bonded; real T-nuts slip; 4.3 kN per plate over 5 bolts is 860 N; assumed T-nut friction is ~1 kN. 860/1000 = 0.86. I reproduced 860 and the bearing stress, 860/(5×3) = 57 MPa. I did not reproduce 4.3 kN from the 5.3 kN + 1.2 kN fall loads.  
57 MPa will not bear out a 3 mm 6061-T6 plate (yield ~276 MPa). The holes are on slot centerlines (post centers X 65 and X 103, rail mid-height Y 363.62), and the in-plane shear runs along those slots. Capacity is friction until the nuts slide into each other. Upper-post pitch is 15 mm (Y 385, 400, 415). A normal ~10 mm T-nut can travel ~5 mm before it jams. The no-gusset side-fall displacement is 5.3 mm. The gusset does not develop the bonded result (1.8 mm, 99th percentile 100 MPa) until the frame has already racked like the failing case (99th percentile 196 MPa).  
The optional +8 Z-face brackets are the same friction joint. They do not carry this shear. The 38 mm offset does leave a clear face: above the lower post (X 55–75) the top of the shared rail is empty, and below the upper post (X 93–113) the underside is empty.  
Smallest fix: one end anchor per post through that clear face into the post end, so shear is not friction. Keep the nine gusset holes. This finding does not by itself change the plate outline.

**4. Major. The stated fan stack does not fit the bay the cut list freezes.**  
Upper bay: 2×78.4 = 156.8 wide, Z clear 100, Y 373.62 to 475 = 101.38 (spec’s 101.4). Pi occupies Y 376–397. Jetson body Y 410–445 leaves 475−445 = 30 mm. Adding the stated +10 mm fan leaves 20 mm, not 30. The two exhaust fans are drawn Y 448–488, Z −50 to −40 (`frame.py` lines 264–265). The rear top rail is Y 475–495, Z −70 to −50. They meet on the plane Z = −50, and 13 mm of the 40 mm fan (Y 475–488) is masked by that rail. A 40 mm fan does not fit in the 30 mm gap. The upper height is locked by the shoulder at Y 424.86 and the existing top at Y 495, so a taller frame is a different robot.  
Smallest fix: place the fans in the door opening below Y 475, or drop them, before anyone treats this bay as done. Do not lengthen posts to solve it.

**5. Major. The rear door is not one agreed opening, and a closed door does not leave plug room.**  
`torso-frame-README.md` line 167: D1 is |X| < 62 (124 mm) by Y 360–480 (120 mm). `body-shell-manufacturing-README.md` line 63: 121 × 120 × 3. Line 119: 124 wide, Y 360–492 (132 mm), “raised 12 mm for the fans.” The question’s 120 × 132 mm matches none of these. Fans to Y 488 fit a door that ends at 492 and stick 8 mm past a door that ends at 480. The packet’s cross-thread check says D1 extent agrees. It does not.  
Pi ports at Z −71.8 are 1.8 mm outside the rear face Z −70. They do lie in the door footprint (Pi X −37.5 to 19.7, Y 376–397). Jetson is 103 wide and fits a 124 mm door; its board edge is Z −45.5, 4.5 mm short of the inner face, with the opening beyond that. With the door open, both port sets are reachable. With it shut, the shell gap is 8–17 mm (`torso-stepped-README.md` line 140). 1.8 mm proud plus an 8 mm gap is 9.8 mm, which is not a USB plug.  
Flat A keep-out is written Y 440–460 and also “2 mm above the Jetson top.” 445+2 = 447, not 440. As numbered, Y 440–445 cuts the Jetson.  
Smallest fix: one door rectangle. Use the shell’s Y 360–492 if the fans stay, and check real plugs against the 8 mm gap. No extrusion change.

**6. Major. Shared-rail nuts for the posts cannot be slid in from the ends once the posts are on.**  
Shared rail, X −113 to 113, Y 353.62–373.62. Slot use from `frame.py`:

| Slot | What lands there | With posts up |
| --- | --- | --- |
| Bottom | Lower-post brackets, nuts near X ±41 | Posts cover X 55–75 and −75–−55. The center bay is open. |
| Top | Upper-post brackets, nuts near X ±79 | Posts cover X 93–113. Center is open. |
| Outer Z | Gusset X 62, 84, 104 (mirrored) and H3 at X ±35 | Face stays open. |
| Inner Z | Shelf brackets near X ±41, step-side brackets near X ±79 | Shelf covers X 55–75, step rail covers X 93–113. Open gap between them is 18 mm. |

Order that works: park the top- and bottom-slot nuts (or use drop-ins in the center), stand the posts and the four inner-X brackets, then the shelf and step-side rails, then outer-slot nuts, gussets, then H3. H3’s 30 mm bracket ends at X 50; the gusset starts at X 55. Slide-in nuts from the rail end cannot reach X ±41 or ±79 after the posts clamp the slot mouth. An 18 mm gap will not accept the 20 mm nut the generator models.  
The generator’s T-nut is 20 mm along the slot (`frame.py` lines 157–162). On the upper post that overlaps (nuts at 385, 400, 415). On the shared rail the nut at X 104 runs to X 114, past the rail end at 113. A real ~10 mm nut fits the 15 mm pitch with ~5 mm left and stays on the rail.  
Smallest fix: write “drop-in, ≤10 mm” on the shared-rail top and bottom slots and on the upper-post outer slots, and insert or park them before the posts. Do not buy from the generator’s 20 mm solid.

**7. Major. The new shelf-end brackets sweep across the shelf plate if that plate is full depth.**  
Shelf-rail 2028, corner (55, 363.62, 50): leg X 27–55, Z 46–50, Y 353.62–373.62, and a second leg X 51–55, Z 22–50. The plate is X ±55, Y 354.6–372.6. Z was not in the CAD check; the generator assumes Z ±50. If the real plate reaches |Z| > 46, those four brackets go through its corners. The old rails were carried by the continuous posts; these brackets are new.  
Smallest fix: measure the plate’s Z. If it reaches the rail ends, notch the plate or flip the bracket onto the rail’s top or bottom face. The 100 mm rail length stays.

**8. Major. Assumed pitch bolts land on the new 2020 corner bracket.**  
Top inner-X bracket, 20 mm leg, corner Y 475: the leg runs Y 455–475, screw at Y 465, in the upper post’s inner-X slot. The script’s pitch screws are Y 390 and Y 470 in that same slot (`frame.py` lines 203–206). Centers 5 mm apart; Ø5.5 holes would intersect. The packet says this 8×M5 pattern is an assumption. Separately, a 28 mm leg from Y 475 ends at Y 447 and overlaps the wing (Y 400–450) by 3 mm. The 20 mm leg ends at Y 455, 5 mm clear. That part of the change is real.  
Under peak use the bonded frame is at 7 MPa (99th percentile) against 145 MPa (`torso-stepped-README.md` lines 63–64). Shortening one leg does not matter for that load. The same FEA does not model the bracket.  
Smallest fix: mark the real pitch-bracket holes. If any fall in Y 450–475, delete the inner-X top bracket and keep the 2028 on the Z face. Do not guess.

**9. Major. Back-fall crush the spec claims is the case that does not hold.**  
`torso-stepped-README.md` lines 146–150: 30 mm stop, back 99th percentile 194 MPa; 40 mm stop, 146 MPa against a 145 MPa limit. Pads in the frame spec are “30 mm” and “30 to 40 mm” (line 168). The shell doc asks for about 30 mm beyond the shell. The generator uses yet another box (X 58–147, Y 368–481, Z −124 to −77) than the frame spec (X 62–135, Y 370–480, Z −94 to −125). Side fall can use the arm. The back cannot. 30×(194/145) ≈ 40 mm, which is why the 40 mm row exists.  
Smallest fix: make the back stop 40 mm, or drop the claim that the back fall stays under yield. Extrusion lengths do not change. Gusset thickness was sized on the bonded 40 mm run.

**10. Major. Cut the 340s as two cuts. One cut does not make both posts.**  
198.62+121.38 = 320. 340−320 = 20. One saw cut at 198.62 leaves about 138 mm, not 121.38, and the top goes to about Y 512 if that stick is used untrimmed. Two kerfs of ~3 mm still leave ~14 mm of waste, so the stock is enough.  
`torso-2020-extrusion.md` lists 4×340+4×100+4×110 = 2200 mm and no shelf rails. The spec’s 2400 mm and “reuse the shelf side rails” add 2×100 that this cut list does not have. The packet already says the real rail count is unverified. Best packing of 2400 mm into 500 mm sticks leaves one full stick (a 340 cannot share a stick with another 340). 2×226+2×186+2×100 = 1024, which does not fit in 500+300 of offcuts. Buying another 4-pack is right if those shelf rails exist, and harmless if they do not. The stepped README’s “from 2460” does not close: 2400−80−220+1024 = 3124, and 2460 does not.  
Smallest fix: on the cut list, “two cuts; upper post must measure 121.38.” Confirm the two shelf rails with the photo, then buy the pack.

**11. Minor. Shopping quantities are exact or easy to misread. Lengths to buy are fine.**  
M5 screws: 32×2 + 36 + 20 + 8 = 128, and the same count of T-nuts. M5×10 is 64+36 = 100, so the 100-pack has no spare. T-nuts: a 100-pack covers 128 only if the ~32 already in the built brackets are reused (132, four left). 2028s: 16 reused + 14 spare = 30, need 28. 150×300 sheet holds four 58×140 plates (2×58 = 116, 2×140 = 280). Gusset mass is not 10 g: the outline area is about 5220 mm², so one 3 mm plate is about 42 g, and four are about 170 g, not 40 g. M5×16 through 12 mm of PETG leaves about 4 mm before the slot recess and about 2 mm of thread in the nut. Use M5×20 for the hardpoints. None of this changes a cut length.

## The seven questions

**1. Step joint.** One inner-X bracket plus the two gussets is the right place for a side fall, and the +8 Z-face brackets do not fix it. The fall shears every gusset bolt along its slot. Friction is the assumed 1 kN against 860 N, with no stop until the nuts jam (~5 mm on the upper post, in line with the 5.3 mm ungusseted deflection). Add end anchors through the clear rail faces. Do not add eight more T-nut brackets and call the fall carried.

**2. Gusset bolts and R15.** The plate will not bear out at 57 MPa. The T-nuts are the weak part, and only by an assumption that is already over the author’s own “they slip first” line. R15 is large enough to fit and far enough from the holes. It is not shown to pull 297 MPa under yield, and the file on disk is still R8.

**3. Assembly.** See finding 6. Posts first only if the shared-rail top and bottom nuts are already in the slot or are drop-ins. Outer-Z gusset and H3 nuts can go in after the posts. Inner-Z shelf and step-side nuts need the 18 mm gap and a short drop-in, or they go in before those rails. The generator’s 20 mm nut fits none of the tight pitches.

**4. Upper bay.** The 156.8 × 100 × 101.4 bay is real, and the 30 mm above a 35 mm Jetson is real. It is not 30 mm once the +10 mm fan is added, and it is not a home for two 40 mm fans. Pi ports, 1.8 mm proud, are inside the door footprint and are reachable with the door open. They are not plug-compatible with an 8 mm shell gap. The door itself is 120 tall in the frame spec and 132 tall in the shell spec.

**5. 2020 brackets at the top inner-X corners.** Under the peak arm load, no. The case is at 7 MPa. The 20 mm leg is the one that actually clears the wing (5 mm, versus a 3 mm clash at 28 mm). Hold the cut of those four only until the real pitch-hole Ys are known (finding 8).

**6. Shopping list.** Buy the 4-pack, the four 2020 brackets, the 3 mm sheet, and the M5×12 25-pack. The 100 T-nut pack is enough only together with the existing ~32. The 100 M5×10 pack is exact, with no drops. Change M5×16 to M5×20 before buying that 25-pack. Do not buy the three fans until they have a place in the bay. Do not laser the current DXF. The as-built cut list is 2200 mm and has no shelf rails; the spec assumes 2400 mm and two reusable 100 mm shelf rails. The photo still decides which is true. Either way the new 226s and 186s do not come out of one leftover stick.

**7. Hip.** This spec does not add hip work beyond the shoulder line the arm thread already locked (output face 97+38 = 135) and a waist bay that was already 40 mm for a 57 mm motor (17 mm short). The bottom stays Y 155, 150 × 140. H6 left the under-face. The +70 mm section belongs below Y 155 and does not change these cuts. The torso battery option does not fit the lower box (inner 110 × 100; pack 185 × 165 × 90) and would reopen it. Leave the pack in the pelvis. The draft handoff’s 175 mm pelvis inner versus a 185 mm pack is not this frame’s cut, and that handoff is the superseded coordinate system.

## Verified vs assumed

Verified from the files and recomputed: post split 198.62+121.38+20 = 340; new stock 1024; total 3124; bay 156.8 and 101.38; wing clash 3 mm and the 20 mm-leg clearance 5 mm; fastener totals 128; DXF R8 and Ø5.5 hole coordinates; door and pad numbers that disagree; fan Y against the top rail; bracket coordinates used above; gusset area and the 57 MPa bearing figure; 860 = 4300/5.

Assumed, and used: T-nut length ~10 mm (the script’s 20 mm does not match any real 2020 nut I would buy); T-nut friction ~1 kN and the 4.3 kN share; 6063-T5 at 145 MPa and 6061-T6 at ~276 MPa; pitch-bracket envelope is a loose bound, not a solid; shelf-plate Z; built frame includes two 100 mm shelf rails; M5×8-style slot recess ~2 mm. Not used as evidence: the PNG sheets (they are the script), the old handoff axes, and the waist adapter pattern.

**NO-GO for the extrusion and the gussets.** Update the DXF to R15, and clear the real pitch bracket against the step side rails, before any of the four 340 mm posts or the four plates are cut. The 226 / 186 / 100 lengths themselves do not need a redesign unless that interference check fails.
