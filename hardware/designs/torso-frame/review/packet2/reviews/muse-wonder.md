# Adversarial gate review – torso frame rev A

## Verdict

**NO-GO for cutting the four 340 mm posts and ordering the gussets as-is.** The basic two-box arithmetic is internally consistent, but pack retention, board/pack electrical clearance, harness routing, step-joint capacity and cooling contain unresolved blockers that can each force post, rail or gusset geometry to change. Several "fits" are envelope intersections, not clearances.

I read [PACKET.md](/Users/joseph/scratch/frame-gate2/hardware/designs/torso-frame/review/packet2/PACKET.md), [data/README.md](/Users/joseph/scratch/frame-gate2/hardware/designs/torso-frame/review/packet2/data/README.md), [data/parts.json](/Users/joseph/scratch/frame-gate2/hardware/designs/torso-frame/review/packet2/data/parts.json), [data/bom.json](/Users/joseph/scratch/frame-gate2/hardware/designs/torso-frame/review/packet2/data/bom.json), [scripts/frame.py](/Users/joseph/scratch/frame-gate2/hardware/designs/torso-frame/review/packet2/scripts/frame.py), the pack/board/shell/FEA notes and the rendered sheets. No file was created, edited or executed as a generator.

---

## What I verified myself vs what I am assuming

**Verified by recomputation from the files**

- Lower box Y 155 → 353.62 = 198.62 mm; upper Y 373.62 → 495 = 121.38 mm; shared rail = 20 mm; total 155 → 495 = 340 mm. 198.62 + 121.38 = 320.00, leaving 20 mm of a 340 post for kerf/offcut.
- Extrusion total 3,124 mm, new stock 1,024 mm = 2×226 + 2×186 + 2×100. M5 count 64 + 36 + 20 + 8 = 128.
- Pack box from [data/parts.json](/Users/joseph/scratch/frame-gate2/hardware/designs/torso-frame/review/packet2/data/parts.json): `pack` X −47.5..47.5, Y 183..345, Z −45..+34. Bottom-rail top Y 175 → 8 mm below; shared-rail underside 353.62 → 8.62 mm above. Front face Z 34 to carrier rear Z 37.5 = 3.5 mm (0.5 sheet + 3.0 air).
- 13S: 13×3.6 = 46.8 V nominal, 13×4.2 = 54.6 V. RS03/RS02 24–60 V is stated in [data/robstride_README.md](/Users/joseph/scratch/frame-gate2/hardware/designs/torso-frame/review/packet2/data/robstride_README.md). The pack note that supersedes this charges to 4.15 V/cell = 53.95 V, not 54.6 V.
- Gusset polygon in [scripts/frame.py](/Users/joseph/scratch/frame-gate2/hardware/designs/torso-frame/review/packet2/scripts/frame.py) (55,285)-(75,285)-(113,335)-(113,425)-(93,425)-(93,373.62)-(55,373.62) = 5,218 mm²; 3 mm 6061 at 2.70 g/cm³ minus nine Ø5.5 holes ≈ **40.5 g per plate**. "40 g total" in the FEA note is wrong; [data/bom.json](/Users/joseph/scratch/frame-gate2/hardware/designs/torso-frame/review/packet2/data/bom.json) (160 g for 4) is right.
- My own AABB check over `parts.json` (cylinders treated as true cylinders where it matters) produced the genuine clashes in Finding M1 below. AABB-only hits I discarded as false positives: top corner brackets vs the Ø99 RS03 (closest bracket point is 51.1 mm radially from the Y 424.86 axis, radius 49.5 – clears), bracket-to-extrusion and screw-to-T-nut "overlaps" (intended mates), RS03-to-pitch-bracket (the motor mounts in it).

**Assumed, not verified – treat as open**

- 0.5 g/mm extrusion mass, ~1 kN T-nut friction, 40 kg robot, 17 g tip-over, PETG bracket strength, cell-holder stack-up that makes the pack exactly 95×162×79, RS03 connector-flat variant A or B, pitch-bracket bolt size (M5 assumed), waist-adapter bolt pattern, built-frame rail inventory. None of these is measured in the packet.

---

## BLOCKER findings

### B1 – The 2.2 kg pack has no positive retention in any axis (Q1)
Evidence: `pack` Y 183..345, Z −45..+34 in [data/parts.json](/Users/joseph/scratch/frame-gate2/hardware/designs/torso-frame/review/packet2/data/parts.json); `packrail_1/-1` X ±(47.5..55), Y 180..348, Z −50..40; D2 door |X| ≤ 60, Y 175..352, magnet-closed onto the H3 spine-rail lip, per README §5–6.

At the FEA's own 17 g basis the pack is a 2.2 × 17 × 9.81 ≈ **367 N** body, before travel amplification.

- **Forward (+Z):** the pack lid at Z 34 already touches the Nomex sheet (Z 34..34.5). The next thing is the PETG carrier at Z 37.5 and the PCBs. A forward fall drives the pack through the boards – mechanical damage and a short risk in the same event.
- **Backward (−Z):** 25 mm of free travel to the frame rear plane Z −70, with no rail at pack height – the rear rails are only at Y 155..175 and 353.62..373.62. The pack arrives at the 2.4 mm PETG D2 door held by magnets and one M3 lock screw. The shell note itself says "D2 must not be a crush path into the pack", yet here the pack is the projectile and the door is the stop.
- **Sideways (±X):** zero clearance – pack face X ±47.5 against rail inner face X ±47.5. It cannot even be slid in with print tolerance, and sideways load goes into two printed rails whose fasteners are not designed, modelled or counted.
- **Up (+Y):** 8.62 mm to the shared rails, which exist only at Z ±(50..70). The pack centre at Z −45..34 can lift and tilt out of the tray.
- **Down (−Y):** it rests on an undefined ledge. `packrail` is modelled as a solid 7.5 × 168 × 90 mm block – that volume in PETG is ~140 g, yet `mass_g` says 40 g. The real rail profile, ledge width and screw pattern do not exist yet.

A printed tray rail plus a magnet door is therefore **not enough in any direction except statically down**.

*Smallest fix:* keep the slide-out concept, add (a) a bolted rear cross-stop inside D2 spanning between the rear posts, not attached to the door; (b) a front bumper/stop that loads the posts, not the carrier – move the carrier or add a separate pack front stop with ≥5 mm of compliant pad; (c) one transverse strap or top retainer over the pack tied to both tray rails; (d) close the ±X gap to a defined 0.5–1 mm/side with the pack located on the ledge. Re-run the slide path with the stops removable from the rear.

### B2 – Board-to-pack and board-to-frame electrical spacing is a crash short waiting for a path (Q2)
Evidence: `pdb_sheet` Z 34..34.5, `pdb_carrier` Z 37.5..41.5, `pdb`/`pdb_logic` Z 43.5..61.5, X ±50, Y 190..340; frame front plane Z 70; PDB design note §1a/§2.1 in [data/pdb_design_README.md](/Users/joseph/scratch/frame-gate2/hardware/designs/torso-frame/review/packet2/data/pdb_design_README.md).

- The sheet is described as "0.5 mm Nomex over the pack lid at Z 34" but the sheet box X ±50 is **wider than the pack X ±47.5** – its edges are unsupported and, per M1, they intersect the tray rails. Nomex protects against a flat lid; it does nothing for the pack corner, a lifted ring-lug, or a carrier that flexes. PETG carrier creep/sag of 3 mm closes the entire air gap.
- M4 REDCUBE-class terminals stand 5–8 mm proud of the board. On the component face they eat into the 8.5 mm to Z 70; a ring lug plus screw head plus a 10 AWG bend cannot turn inside 8.5 mm – the cable will bear on the front posts, the intake fans at Z 70..80, or both.
- The over-pack run (`harness_pack` Y 345..353.6) has **8.62 mm total** for a 10 AWG pair (≈5.5–6 mm OD each, side-by-side only, never stacked), the sense harness, **and** the lug bend at the HP top edge. The PDB note's own layout constraint ("ring lugs and the 10 AWG bend stay inside Y 340..353.6, Z 43.5..60") describes a bend radius that 10 AWG silicone cannot make.
- Fault behaviour: a chafed pigtail, the aluminium-clad brake resistor, or an RS03 housing can put the frame at bus potential. The proposed single-point bond of 1 MΩ ∥ 10 nF passes ≈55 µA at 54.6 V – **no fuse will ever blow**, the frame stays live indefinitely, and a second fault (e.g. through a motor housing – the packet itself notes "the frame touches the actuator housings anyway") shorts the pack through the frame. The 10 nF leg and its PCB trace become the fuse. The bond is an ESD bleed, and the packet treats it as if it were fault protection.

*Smallest fix:* increase pack-to-carrier spacing or add a rigid insulating pack lid/standoff so the gap survives carrier flex; put a mechanical cable clamp at each M4 terminal so a lug cannot rotate into the pack or frame; shorten/reroute the over-pack run (e.g. sense + power enter at the pack rear corner only, with a service loop at D2 – already half-specified, make it the only path); either bond the frame solidly at one point with a wire sized to blow the pack fuse, or fully insulate every credible frame contact – the 1 MΩ middle state is the worst of both. Decide which, and write the fault case down before layout.

### B3 – Two modelled "cable paths" pass through solid extrusion (Q3, Q4)
Evidence: `harness_step` X 33..45, Y 340..376, Z 50..60; `rail_shared_1` X −113..113, Y 353.62..373.62, Z 50..70; `shelf` X ±55, Y 354.6..372.6, Z −50..50.

The claimed LV path "up through the gap between the shelf plate and the shared front rail (Z 50–60)" does not exist: the shelf plate fills Z −50..50 and the front shared rail fills Z 50..70 over the same Y 354..373 band. The reserved box intersects the rail by 12 × 20 × 10 mm. The front-corner trunks `harness_down_±1` (Ø8 at X ±51, Z 66, Y 160..370) similarly pass straight through the bottom front rail (Y 155..175, Z 50..70) and the shared front rail. As modelled, there is **no continuous route from the boards to the Pi/Jetson or to the waist** except around the shelf-plate ends, which are occupied by the shelf rails (X ±55..75) and pitch brackets.

*Smallest fix:* pick the real route now and model it as a swept path with bend radius – the credible option is inboard of the front posts at Z 40..50 (the existing side-band) rising behind the shelf plate, or a defined notch in the shelf plate (the "withdrawn" cutout). Whichever it is, re-run the interference check on the swept volume, not on straight cylinders.

### B4 – Step joint: friction-only, and the pack makes the demand side worse, not better (Q7)
Evidence: FEA note in [data/torso_stepped_FEA_README.md](/Users/joseph/scratch/frame-gate2/hardware/designs/torso-frame/review/packet2/data/torso_stepped_FEA_README.md) – 4.3 kN per plate, ≈860 N per bolt against "~1 kN assumed" T-nut friction; README §10 lists the end anchor as proposed-not-designed.

The margin is 860 N demand vs ~1,000 N assumed capacity with **no factor for preload relaxation, slot tolerance, black-anodise friction, or repeated falls** – and the FEA that produced 860 N was run on the earlier plate with 5 bolts, while the release plate has 9 holes (X 65 Y 297/317/337; Y 363.62 X 62/84/104; X 103 Y 385/400/415) whose load share is not uniform: the three shared-rail holes carry the step transfer, the post holes mostly clamp. Adding 2.2 kg at Y 264 plus boards and rails raises the mass below the step that the joint must arrest in a side fall; it also raises total fall energy above the 40 kg basis (2.2 kg ≈ +5.5 % before boards/harness). The lower centre of mass helps the *global* tip-over moment slightly; it does not reduce the *local* shear the gusset slots see when the lower box decelerates – that scales with lower-box mass. The open item is therefore mildly worse than stated, and it is the joint the whole gate turns on.

**Ordering gussets before the end anchor is designed risks scrapping them:** any anchor that uses the post end faces or the gusset outline (longer plate, extra hole row, larger R at X 113/Y 335 where the 297 MPa peak sits) changes the DXF. The DXF in [data/gusset_plate_left.dxf](/Users/joseph/scratch/frame-gate2/hardware/designs/torso-frame/review/packet2/data/gusset_plate_left.dxf) is a single left-front plate with absolute torso coordinates; a vendor still needs part-local origin, thickness callout and the mirrored pair defined.

*Smallest fix:* design the positive anchor first (e.g. an internal joining plate or through-bolt tying each lower-post top end to the shared rail, so slip is carried in bearing, not friction), fold any extra holes into the plate, then cut gussets. A one-evening T-nut pull/slip test on offcut 2020 would replace the "~1 kN assumed" with a number.

---

## MAJOR findings

### M1 – Genuine interferences and <2 mm clearances from my own check of `parts.json` (Q3)
Intended mates excluded (corner brackets in corners, screws/T-nuts, RS03 in its pitch bracket, hardpoint brackets on their rails).

| Pair | Overlap / gap | Coordinates |
|---|---|---|
| `pdb_carrier` × `packrail_±1` | **overlap 2.5 × 150 × 2.5 mm** | X ±(47.5..50), Y 190..340, Z 37.5..40 |
| `pdb_sheet` × `packrail_±1` | **overlap 2.5 × 150 × 0.5 mm** | Z 34..34.5 – the carrier cannot be fitted with both tray rails in place |
| `harness_step` × `rail_shared_1` | **overlap 12 × 20 × 10 mm** | see B3 |
| `harness_down_±1` × `rail_bot_1`, `rail_shared_1` | **overlap Ø8 through both front rails** | X ±(47..55), Z 62..70 |
| `harness_shoulder_±1` × `jetson` | **overlap 9.5 × 8 × 10 mm** | X ±(42..51.5), Y 447..455, Z −32..−22 – inside the RS03 flat-A keep-out the README itself declares (X ±50..78, Y 440..460, Z −45..−15) |
| `br24..br31` (step/shelf brackets) × `shelf` | **overlap up to 28 × 18 × 4 mm** | shelf plate X ±55, Y 354.6..372.6 runs into all eight step-tier bracket legs |
| `br3/br9/br15/br21` × `pitch_wing_±1` | **overlap 4 × 1.62 × 19 mm** | Y 400..401.62, X ±(89..93) – the 2020-leg fix protected the top brackets (Y 447..450), not these |
| `spine_rail` × `gusset_±1_−1` (rear) | **overlap 5 × 17 × 3 mm** | X ±(55..60), Y 355..372, Z −73..−70 |
| RS03 body × upper posts / step & top side rails | **0.5 mm radial** | Ø99 in the 100 mm inner depth, Z ±49.5 vs faces at Z ±50. Genuine but accepted "as built"; any extrusion bow or bracket screw head in that plane closes it |
| `pack` × `packrail_±1` | **0.0 mm** | X ±47.5 faces in contact – no slide tolerance |
| `pdb`/`pdb_logic` × lower-post top & bottom brackets (`br0/br2`…) | **1.0 mm in X** | board edge X ±50 vs bracket leg X 51..55 |
| `shelf` × lower-post tops | **0.98 mm in Y** | shelf underside 354.6 vs post top 353.62 |
| `pi5` × H3 rear brackets | **2.0 mm in Y** | Pi Y 376.. vs H3 top 374, with Pi ports already at Z −71.8, 1.8 mm past the rear face |
| `brake_res` × `packrail_-1` | **0.0 mm in Y** | resistor top Y 180 = rail bottom Y 180 (and see M4) |

Claim 6 in the packet ("every listed envelope is free of interference except where the README says otherwise") is **not true** for the carrier/tray-rail, harness, Jetson-harness and shelf-bracket pairs.

### M2 – Airflow path is not physically continuous (Q5)
Intakes `fan_intake_hp` Y 264..304 and `fan_intake_lp` Y 193..233 sit at Z 70..80, blowing at a board face 8.5 mm away (Z 61.5) inside a box walled by posts (X ±55), pack (rear), shared rail (top) and waist adapter (bottom). The only exits are the 10 mm side bands Z 40..50 – which are also the pigtail routes – because the shelf plate seals the top of the lower box and B3 removes the claimed cable-slot chimney. The two exhausts (Y 430..470, Z −80..−70) are in the **upper** bay, in the D1 door plane, downstream of the Jetson (Y 410..455, Z −45.5..45) which blocks most of the upper-bay cross-section and has no designed shelf. Lower-box heat therefore has no defined path to the exhausts; the lower box will pressurise and leak through D2 and panel seams – across the pack.

Worst hot spots, in order: (1) the BMS FET bank – the floorplan puts FETs at Y 294..318 but the intake jet covers Y 264..304, an overlap of only 10 mm, and the PDB thermal note contradicts the floorplan by placing the DC-DC in that same jet; one of the two banks is outside the jet whichever note is right; (2) the Jetson, recirculating in a sealed upper bay against its own 20 mm clearance to the top rails; (3) the brake resistor (M4), dumping clamp energy into a bottom side rail directly under the PETG tray rail and the pack's lower corner. The shell note's 60–75 W chest estimate vs the PDB's ~18 W board loss + RS03 losses are never reconciled.

*Smallest fix:* decide the route (lower-front intake → across both boards → rear lower exhaust or a ducted rise at the rear corners past the pack), move one exhaust into the lower bay, and align fan Y bands with the final FET/DC-DC Y bands on one controlled floorplan revision.

### M3 – Serviceability walk-through fails at the connectors (Q4)
Pack out: open D2 (lock screw, magnets), reach the XT90S and 2×9 Micro-Fit at the pack rear top corner – modelled at Z −55..−45, i.e. 15 mm inside the rear face, at Y 318..343, only **9 mm below the door's top edge (Y 352)**. That is a two-finger locking-connector release in a slot, followed by a **104 mm** straight pull to clear the rear face (the shell note says "~90 mm", which leaves the pack front at Z −56, still inside). It is doable exactly once per careful operator, with the 30 mm board-side slack as the only help, and the sense board (X 5..45) must not snag the tray-rail rear ends at Z −50 – it protrudes to Z −55, 5 mm past them, at X clear of the rails, so this one is actually fine.

Boards out: they cannot come out forwards – carrier X ±50 between posts at X ±55 with unmodelled mounting tabs, 8.5 mm of forward travel, ring lugs that must be unbolted in that 8.5 mm gap, fans at Z 70..80 in the U3 panel directly in front, arm pigtails dressed in front of the board face, the over-pack power/sense run, the HP-LP ribbon, and LP bottom-edge connectors all still attached. Realistically: U3 off, fans disconnected, pack out first via D2, then disconnect everything from behind, then carrier. Neither the pack-first dependency nor fan disconnects are in any assembly order in the packet.

### M4 – Brake resistor has three different homes, one of them touching the battery tray
[scripts/frame.py](/Users/joseph/scratch/frame-gate2/hardware/designs/torso-frame/review/packet2/scripts/frame.py:237) comment says "front bottom rail"; the same file's part text and [data/README.md](/Users/joseph/scratch/frame-gate2/hardware/designs/torso-frame/review/packet2/data/README.md) §6 say −X bottom **side** rail, `brake_res` X −55..−47, Y 155..180, Z ±25; [data/pdb_PLAN.md](/Users/joseph/scratch/frame-gate2/hardware/designs/torso-frame/review/packet2/data/pdb_PLAN.md) records the location as "closed" on the front bottom rail at X ±25, Y 153..178, Z 52..60. As modelled it sits 5 mm proud above its own rail (rail top Y 175, part to Y 180), in contact with the pack tray rail above it, 0.5 mm in X and 3 mm in Y from the pack corner, heating the rail the PETG tray sits on. A 10 Ω clamp resistor at 57.5 V dissipates 330 W while on. Pick one location, put it in all three documents, and keep it off the pack tray rail.

### M5 – Shopping list / BOM will strand the build (Q6)
- [data/bom.json](/Users/joseph/scratch/frame-gate2/hardware/designs/torso-frame/review/packet2/data/bom.json) item 16 has **qty 12, 377 g** – the generator counted carrier, sheet, brake resistor, Pi stub and six screws as twelve "power and logic boards". Item 32 has **qty 2, 2,212 g** – pack + sense board counted as two packs. Item 27 qty 8 counts cable-path boxes as trunks. Anyone ordering from the BOM orders nonsense.
- Fans: shopping list says buy **3**, parts list and generator model **4** (2 intake + 2 exhaust), PDB provides **3** fan headers.
- Corner brackets: §2 needs 24× 2028 + 8× 2020 (matches `parts.json`); §7 says "32 total: 28 × 2028 + 4 × 2020". The shopping list says buy no 2028s because 16 are "used" – but the reference photos show the built bottom corners are **black printed gusset brackets and metal angle brackets, not aluminium 2028s** (see [built_frame_bottom_front_20261106.jpg](/Users/joseph/scratch/frame-gate2/hardware/designs/torso-frame/review/packet2/data/reference/built_frame_bottom_front_20261106.jpg); README §9 concedes this). The 14 "spares" may not exist.
- Not counted anywhere in the 128 M5: pack tray rail screws, carrier-to-post mounting (the carrier X ±50 does not even reach the post faces at X ±55 – no tab is modelled), Jetson shelf and its fasteners, fan fixings, spine-pad metal standoffs that the shell note requires to be "clamped through to the rail", D2 hinge/magnets, extrusion end treatment. No slot-by-slot nut map exists (packet admits this); with slide-in nuts assigned to corner brackets, several slots become unreachable once rails close the loops – the drop-in quantity (50) vs drop-in demand (36 gusset + 8 pitch + 4 H3-rear = 48) leaves 2 spare for every mistake.
- Pitch-bracket screws are M5×12 by assumption; the arm-thread CAD note says bolt size "is not in the CAD (M5 or M6), read it off the built robot". If they are M6, eight screws, eight T-nuts and possibly the bracket ears are wrong.
- Gusset and bracket screws are M5×10 through 3 mm plate / 4 mm bracket into a T-nut behind a 2020 face – thread engagement is a few millimetres at best. Nobody has checked the actual stick-out; this is the same joint as B4.

### M6 – Stale and contradictory source text inside the packet
- [data/README.md](/Users/joseph/scratch/frame-gate2/hardware/designs/torso-frame/review/packet2/data/README.md) §6 closes with "The frame is adequate… **with the main battery in the pelvis**. If Joey wants the main pack in the torso, the lower box is too small" – directly contradicting the locked 13S2P-in-torso decision two pages earlier and claim 3 of the packet. §9 item 2 still lists battery location as an open Joey decision.
- Pack voltage: generator/README use 54.6 V full; the pack build-plan update (P50B, charge 4.15 V) gives 53.95 V; the regen clamp at 57.5 V then has 3.5 V of headroom, and to the RS03 60 V ceiling only 2.5 V, with clamp tolerance and TVS behaviour unverified. "No regulator is needed" is true for the drives only because the clamp exists – it is a designed part, not a margin.
- Frame mass: "about 2.1 kg including brackets and T-nuts" – summing [data/bom.json](/Users/joseph/scratch/frame-gate2/hardware/designs/torso-frame/review/packet2/data/bom.json) items 1–11, 34–39 gives **2.52 kg**. "Everything shown: about 7 kg" vs a BOM total of 9.52 kg (including the double-counted pack and waist RS03). The tip-over basis (40 kg, crush distances) is sensitive to which number is real.
- Exhaust fans, intake fan count ("1 intake" in the packaging table vs 2 in the parts list), and the third-board "yes, lower box front" vs "no home yet" all appear in both states.

---

## MINOR findings

- **m1 – Duplicated IMU.** `imu_old` (X 30..50, Y 376.62..382.62, item 15) and `bno085` (X −50..−30, same Y, item 33) model the same sensor twice on the shelf; the BOM silently drops one. `pi_stub` (X 22..52, Z 20..40) floats 1.02 mm above the shelf plate with no standoff modelled.
- **m2 – Standoff keep-outs exceed the boards.** 6 mm keep-out pads at X ±46, Y 194/265/336 on a X ±50 board put pad edges at X ±52 and Y 342 – off the PCB at the corners. harmless if redrawn, wrong if the floorplan is used literally.
- **m3 – Cut tolerance is unbudgeted.** 198.62/121.38 imply 0.01 mm intent on a saw cut; shoulder axis Y 424.86 sits 51.24 mm above the upper-post bottom, inside the pitch-bracket slot range Y 416.6..432.8, so ±1 mm is absorbable – but no tolerance is stated, and the step stack (post top 353.62 = rail underside) has no shim/gap allowance. State ±0.5 mm and which face is the datum.
- **m4 – Hardpoint bracket retention.** Each 30×20×12 PETG bracket is held by a **single** M5 (generator) while the shell note budgets 40× M5×20 for 20 brackets. A single-screw bracket can rotate under panel load; either add the second screw/pin to the count (128 → 148 M5) or show the anti-rotation feature.
- **m5 – RS03 depth margin.** The accepted 0.5 mm/side (M1) also applies to the rear boss Ø80 and to any connector fitted in the 11–12 mm radial room; connector flat A vs B is still unknown (packet open item) and flat A already collides with the Jetson harness (M1). Do not order harness or the Jetson shelf until Joey looks at both motors.
- **m6 – D2/hands clearance.** Spine pads flank D2 with inner edges at |X| 73 vs door |X| 60 – 13 mm finger room each side for a 2.2 kg pack pull. Acceptable, but the pack pull test should be done with the pads fitted, not on the bare frame.

---

## Direct answers

1. **Pack retention:** No – see B1. Nothing positive holds it forward, backward or up; sideways is a zero-gap print. Smallest adequate change is B1's rear cross-stop + front post-mounted stop + top strap, with the door explicitly non-structural.
2. **Electrical-mechanical safety:** The credible crash event is pack → carrier → board, plus lug rotation and chafe in an 8.62 mm cable slot – see B2. The 1 MΩ ∥ 10 nF bond makes it worse in one specific way: a frame fault draws ~55 µA, never clears, and waits for a second fault to short the pack through the frame. Replace with either a fuse-clearing solid bond or true insulation of every contact.
3. **Interference:** Claim 6 fails. Full list with ids and coordinates in M1; the worst are carrier×tray-rails, harness×shared-rail, harness×Jetson. My check was AABB/cylinder over `parts.json`; it cannot see screw heads inside slots, print tolerances, or cable bend radii – all of which only shrink the clearances quoted.
4. **Serviceability:** Pack: possible but fiddly (M3) – 9 mm connector access, 104 mm pull, pack-first dependency for everything else. Boards: not removable as an assembly without removing the pack and disconnecting every pigtail; ring lugs cannot be unbolted in the 8.5 mm front gap. Walk-through in M3.
5. **Cooling:** Not plausible as drawn (M2). The lower box is sealed at the top by the shelf plate; exhausts are in the wrong bay; one of the FET/DC-DC banks is outside the intake jet under either floorplan revision. Worst hot spot: BMS FET bank, then the Jetson.
6. **Build:** Cut plan arithmetic is right; the shopping list is not (M5). Joey would find missing: tray-rail and carrier fasteners, a Jetson shelf, fan fixings and a fourth fan, possibly all sixteen "reused" 2028 brackets (photos show printed corners), possibly M6 pitch hardware, and two T-nuts of margin. The built frame being a bare single-rail box (owner-confirmed) also means the "existing" shelf side rails that become the step side rails need a physical count before cutting – if they are absent, two more 100 mm rails come out of the same new-stock pack, which still covers it.
7. **Step joint:** Worse than stated, modestly – lower-box mass and total energy both rise, margin was ~860 N vs ~1,000 N *assumed* friction. Do not order gussets before the end anchor exists; the anchor can change the plate (B4).
8. **What makes cutting/ordering a waste, right now:** cutting the posts is irreversible, and any one of these reopens the geometry – the end-anchor design (gusset outline/holes), the pack retention stops (tray and possibly rear-post hardware), the board carrier redesign for spacing/serviceability, the harness route (shelf notch or reroute), the Jetson shelf, the waist/pelvis decision (adapter pattern unverified; RS03 waist is 23 mm short of its 40 mm bay), or a physical inventory showing the shelf/step rails or 2028 brackets are not on hand. The gussets are the cheap mistake (~40 g plates, one 150×300 sheet); the 340 mm posts are the expensive one – once cut to 198.62 + 121.38 there is no single-box frame to go back to. Cut posts last, after B1–B4 have paper fixes and the bench inventory is counted.
