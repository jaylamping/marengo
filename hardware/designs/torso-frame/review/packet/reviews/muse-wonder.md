**Gate verdict up front: do not cut yet as one release.** [New-stock extrusion lengths](J:\code\marengo\hardware\designs\torso-frame\review\packet\data\cutlist.md:13) are arithmetically sound. Cutting the four built 340 mm posts and cutting gussets are both **NO-GO** today, for different reasons below.

## What I verified myself (recomputed, not copied)

- Post split: 198.62 + 121.38 = 320.00 mm from a 340 mm post. 20 mm remains for kerf and the offcut. Step heights in [frame.py](J:\code\marengo\hardware\designs\torso-frame\review\packet\scripts\frame.py:14) (`Y_BOT 155`, `Y_STEP_LO 353.62`, `Y_TOP 495`) reproduce both lengths exactly.
- Extrusion total: 3124 mm. New stock 1024 mm (2×226 + 2×186 + 2×100). One 4×500 mm pack yields it in 3 sticks (452 mm, 472 mm, 100 mm). Sheet fit also checks: four 58×140 mm gusset blanks fit a 150×300 mm sheet as 116×280 mm.
- Inner clears 110×100 mm (lower) and 186×100 mm (upper), 38 mm step, 156.8 mm between RS03 rear faces, and 26.9 mm per side around the 103 mm Jetson all recompute correctly from [torso-frame-README.md](J:\code\marengo\hardware\designs\torso-frame\review\packet\data\torso-frame-README.md:30).
- Fastener arithmetic: 32 brackets × 2 = 64, gussets 36, hardpoints 20, pitch brackets 8. Total 128 M5. [parts.json meta](J:\code\marengo\hardware\designs\torso-frame\review\packet\data\parts.json:1) agrees (128/128/16).
- The 2028-to-2020 swap rationale checks geometrically: a 28 mm leg reaches Y 447 at the top post. The pitch wing ends at Y 450. That is the stated 3 mm overlap. A 20 mm leg starts at Y 455, leaving 5 mm.

## What I am assuming (packet-declared, not evidence)

T-nut friction ≈1 kN, 0.5 g/mm extrusion mass, RS03 connector flat A/B, PDB 100×90×28 mm envelope, pitch-bracket hole pattern, built-frame rail count, and main battery in the pelvis. Every finding that leans on these is marked as such.

## Findings

**1. BLOCKER — Gussets: the file you would cut does not contain the R15 fix. (Q2)**
Spec and generator claim R15 at X 113 / Y 335: [torso-frame-README.md](J:\code\marengo\hardware\designs\torso-frame\review\packet\data\torso-frame-README.md:117), [frame.py GUSSET_R](J:\code\marengo\hardware\designs\torso-frame\review\packet\scripts\frame.py:95). Every arc in [gusset_plate_left.dxf](J:\code\marengo\hardware\designs\torso-frame\review\packet\data\gusset_plate_left.dxf:90) is R8.0, including that corner. Laser-cutting the DXF as instructed in the shopping list produces the pre-fix plate at the exact corner where the arm-thread FEA peaks at 297 MPa. *Smallest fix: regenerate the DXF with R15 at (113, 335), re-export, and checksum it. Do not hand-edit radii at the cutter.*

**2. BLOCKER — Pitch-bracket screws collide with corner-bracket T-nuts in the same slot, and M5×12 may not even reach the post. (Q3, Q6)**
Upper-post inner-X slots must hold: the step bracket nut (≈Y 377–398), the top bracket nut (≈Y 455–475), and pitch-bracket nuts at Y 390 and Y 470 ([frame.py pitch screws](J:\code\marengo\hardware\designs\torso-frame\review\packet\scripts\frame.py:203)). Both pitch positions overlap a bracket nut in the same 20 mm-wide slot run. Physically they cannot coexist as drawn. Separately, the screws are modelled starting at X ±78.4 with 12 mm length. The post face is at X ±93. That is a 14.6 mm stand-off ([torso-frame-README.md](J:\code\marengo\hardware\designs\torso-frame\review\packet\data\torso-frame-README.md:151)). A 12 mm screw starting at the motor plane stops 2.6 mm short of the post. The packet already flags this pattern as an unverified assumption ([PACKET.md](J:\code\marengo\hardware\designs\torso-frame\review\packet\PACKET.md:37)). *Smallest fix: measure the built bracket (entry face to post face, and true hole Y positions) before cutting posts. Move the upper step/top connection on that slot to the post Z-face — which also answers Q1 — or shift pitch fixings to drop-in nuts offset from bracket nuts. Re-spec screw length from the measurement, likely M5×16/20, not ×12.*

**3. BLOCKER — The reuse plan assumes two 100 mm shelf rails that the built BOM does not list. (Q6)**
Rev A needs eight 100 mm side rails (bottom 2, step 2, shelf 2, top 2). Item 5 is “the built shelf side rails” ([torso-frame-README.md](J:\code\marengo\hardware\designs\torso-frame\review\packet\data\torso-frame-README.md:112)). The built cut list and master BOM contain only four 100 mm rails total and twelve extrusions overall: [torso-2020-extrusion.md](J:\code\marengo\hardware\designs\torso-frame\review\packet\data\torso-2020-extrusion.md:16), [master-bom.csv](J:\code\marengo\hardware\designs\torso-frame\review\packet\data\master-bom.csv:8). The generator’s “built” model quietly includes two shelf rails, giving 2400 mm built versus 2200 mm in the extrusion note. If those rails do not exist, the plan is short two 100 mm rails. The new 4-pack still covers them, but only if Joey knows before dismantling. *Smallest fix: count and photograph the built frame (already requested in the packet). Until then, treat two extra 100 mm rails as new cuts.*

**4. MAJOR — Q1: one inner-X bracket per post end plus the two gussets is not shown to be enough. Make the optional +8 mandatory at the step.**
The gussets lie in the X–Y plane. They are strong for the side tip-over and nearly useless out-of-plane for the forward tip-over, where 3 mm plate bending over a 140 mm span carries little. Forward and torsional load then hangs on one bracket line per post end, in T-slot friction. The numbers also disagree with each other: the packet question uses 860 N per bolt, which comes from “4.3 kN per plate over 5 bolts” ([torso-stepped-README.md](J:\code\marengo\hardware\designs\torso-frame\review\packet\data\torso-stepped-README.md:83)), while the plate has 9 bolts. Shared over 9, the load is ≈478 N. Nobody states which bolts are effective. At 860 N against an assumed ≈1 kN friction limit, preload loss or vibration slips the joint onto the bolts. Bearing itself is fine (860 N over 5×3 mm ≈ 57 MPa), and tear-out ligaments are 6–7 mm, adequate statically. Slip and slot-lip pry are the failure modes, and the FEA cannot see them because joints are bonded ([torso-stepped-README.md](J:\code\marengo\hardware\designs\torso-frame\review\packet\data\torso-stepped-README.md:49)). *Smallest fix: add the Z-face brackets at all four step post-ends (the costed “optional +8”), which also frees the inner-X slot for Finding 2. Add 16 M5×10 and 16 T-nuts to the buy list.*

**5. MAJOR — Q3: no slot map exists, and slide-in nuts will trap the design.**
Front outer slot of each shared rail alone must hold 6 gusset nuts plus 2 H3 nuts (160 mm of nut in a 226 mm slot: feasible, but only if preloaded). Once the step ladder is closed by the side rails at X ±93–113, top and bottom slot ends are blocked. Gusset nuts in the vertical post slots are likewise trapped once posts meet rails, yet the gussets are the part Joey must remove to inspect T-nuts after a fall. The shopping list says “slide-in or drop-in” without assigning types. *Smallest fix: publish a per-slot preload map and a build sequence (preload shared rails and posts on the bench, build the step ladder flat, then raise the boxes). Specify drop-in nuts for gussets, H3, and pitch brackets (64 minimum) for serviceability; keep slide-ins for corner brackets assembled open.*

**6. MAJOR — M5×16 hardpoint screws have almost no thread engagement. (Q6)**
16 mm screw − 12 mm PETG bracket = 4 mm past the bracket face. The extrusion slot wall consumes roughly half of that before the T-nut thread starts. Engagement is on the order of 1–2 mm. These same brackets carry the TPU spine pads in a rear fall, so this is a crash load path, not a trim screw. *Smallest fix: change the 25-pack to M5×20 and verify the tip does not bottom in the 2020 centre slot on a physical coupon. Do not buy the M5×16 pack.*

**7. MAJOR — Q4: the upper bay fits on paper and is not serviceable as specified.**
Stack arithmetic is correct (Pi top Y 397, Jetson Y 410–445, 30 mm to the top-rail underside). But the Jetson’s second shelf is an undesigned bracket cantilevering ≈41 mm per side from the posts, with no fasteners in the BOM. Two 40 mm exhaust fans are placed at Y 448–488 ([frame.py fans](J:\code\marengo\hardware\designs\torso-frame\review\packet\scripts\frame.py:264)): a 40 mm fan cannot live in the claimed 30 mm plenum, so it rides 13 mm into the top-rail band, 3 mm above the Jetson, under a harness trunk 2 mm above that. Extraction is a 2–3 mm threading exercise. The Pi’s ports sit 1.8 mm proud of the frame face at Z −71.8 against a 10–17 mm shell gap: a straight plug body plus boot (≈20 mm) does not fit with the shell closed, which the shell plan already lists as unresolved. The door itself is specified two ways: Y 360–480 in [torso-frame-README.md](J:\code\marengo\hardware\designs\torso-frame\review\packet\data\torso-frame-README.md:167) versus Y 360–492 in [body-shell-manufacturing-README.md](J:\code\marengo\hardware\designs\torso-frame\review\packet\data\body-shell-manufacturing-README.md:119). At 480 the fan tops are behind shell, not door. *Smallest fix: design the Jetson shelf bracket and add its fasteners; mandate right-angle Pi cables; reconcile D1 to 360–492 everywhere. None of this changes extrusion lengths.*

**8. MAJOR — Tip-over survival routes crash load through printed shell brackets. (Q1/Q2 system effect)**
The frame needs ≈30 mm side and ≈40 mm rear crush; the shell alone gives 10–17 mm at the back, where the 99th percentile reaches 584 MPa against a 145 MPa limit ([torso-stepped-README.md](J:\code\marengo\hardware\designs\torso-frame\review\packet\data\torso-stepped-README.md:144)). The remedy is TPU pads standing on H2/H3/H4 PETG brackets with the under-engaged screws of Finding 6. Those brackets were designed as shell mounts with a tear-out fuse order, not as primary crash structure. If a bracket slips or tears, the frame sees the 10 mm case. *Smallest fix: give the pads a metal load path (standoff to rail with through-clamp, not only the PETG bracket) and validate with coupon C9 plus a bracket pull test. This does not block extrusion cutting, but nobody should claim tip-over protection until it is done.*

**9. MAJOR — Q7: the frame does not make the hip stage harder, except for one unreserved intrusion.**
Keeping the 150×140 mm bottom rectangle and moving H6 to the outer side faces ([mounts.json](J:\code\marengo\hardware\designs\torso-frame\review\packet\data\mounts.json:124)) is the right call and frees the under-face. Two leftovers make hip work harder than the spec admits. First, the waist RS03 as modelled spans Y 92–149, protruding ≈23 mm into the 115 mm pelvis where the 90 mm-tall main pack is meant to sit; no pelvis volume is reserved for motor plus adapter. Second, the in-torso battery option (Y 175–330) would occupy exactly the space a pocketed waist adapter would need. Those options are mutually exclusive and the spec only prices the PDB conflict. *Smallest fix: freeze “battery in pelvis, no torso pack” before cutting (already the baseline), and add the waist-motor intrusion volume to the pelvis/hip-stage envelope. No frame dimension changes.*

**10. MINOR — Q5: the four 2020 top brackets are acceptable. (can wait)**
Peak-use FEA sits at a 7 MPa 99th percentile, two orders below yield, and that case is bonded-joint optimistic but not by 20×. Shorter 20 mm legs cost pry stiffness at a corner that is not the tip-over fuse; the step (Finding 4) and top-rail joints will move first. *Fix: none before cutting. Add top-corner inspection to the post-fall checklist.*

**11. MINOR — Mass bookkeeping is wrong in the optimistic direction. (can wait)**
Gusset mass is modelled as 10 g per plate ([frame.py](J:\code\marengo\hardware\designs\torso-frame\review\packet\scripts\frame.py:101)). Polygon area is ≈5218 mm²; at 3 mm in 6061 that is ≈40 g per plate, ≈160 g per set, not 40 g total. The “≈2.1 kg frame” is ≈2.29 kg corrected, before T-nut mass the BOM filters out. *Fix: set 40 g per plate in the generator and regenerate the BOM.*

**12. MINOR — Shopping list gaps beyond Findings 4–6. (Q6, can mostly wait)**
M5×10 buy quantity is exactly the 100 needed (64 + 36), with zero spares for dropped hardware. Fan screws, Jetson-shelf fasteners, harness T-slot clips, and M2.5 standoffs are used but not bought. The RS03 still has only 0.5 mm per side in the 100 mm depth, unchanged from as-built, so it is a proven-but-zero-tolerance fit: any extrusion bow or bracket flash becomes an assembly force-fit. *Fix: buy T-nuts as 100 slide-in plus 64 drop-in, add one spare M5×10 pack, and list fan/shelf/clip hardware once the Jetson bracket exists.*

## Direct answers

| Question | Answer |
| --- | --- |
| Q1 step joint | Not proven enough. Add the +8 Z-face brackets (Finding 4). |
| Q2 gusset bolts / R15 | Bearing and tear-out pass on paper. Slip margin does not. R15 is irrelevant until the DXF actually contains it (Finding 1). |
| Q3 assembly / slots | Buildable only with a preload map and drop-ins where service is needed. One hard collision exists (Finding 2). |
| Q4 packaging | Statically fits. Not serviceable through the door as specified (Finding 7). |
| Q5 2020 top brackets | Acceptable. No change (Finding 10). |
| Q6 shopping list | Wrong on M5×16 length, drop-in types, spares, and possibly two 100 mm rails (Findings 3, 6, 12). |
| Q7 waist / battery | Frame is neutral except the unreserved pelvis intrusion. Freeze battery-in-pelvis (Finding 9). |

## Gate

- **Extrusion, new stock (2×226, 2×186, 2×100): GO.** Lengths are verified and needed under every open option.
- **Extrusion, cutting the four built 340 mm posts: NO-GO** until the built side-rail count is photographed (Finding 3) and the pitch bracket is measured (Finding 2). Both are bench checks. Neither changes a cut length, but the post cut is the one irreversible step in the packet.
- **Gusset plates: NO-GO.** Regenerate the DXF with the R15 corner (Finding 1) and re-checksum before any plate is cut.

Overall for the release as written: **NO-GO.**
