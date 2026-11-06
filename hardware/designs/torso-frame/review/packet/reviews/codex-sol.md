I verified the packet’s coordinates, part geometry, DXF entities and arithmetic. The recorded SolidWorks clearance checks and FEA results remain unverified claims: the packet contains neither the native assembly nor sufficient solver evidence to reproduce them. Actual stock, connector variants, fastener engagement and joint capacities remain assumptions.

References below are packet-relative.

1. **Blocker — Q1: the four lower-post step brackets do not attach to the lower posts.**  
   `scripts/frame.py:127,137` selects the upper width whenever `y == Y_STEP_LO`. Consequently, `data/parts.json`, part `br2`, places its vertical leg at X **89–93**, while the lower post occupies X **55–75** and needs a bracket against its inner face at X **55**. The other three corners mirror this error. These brackets cannot provide the claimed connection.  
   **Smallest fix:** explicitly select the lower-post width for these four brackets, then recheck their bolts, nuts and surrounding parts.

2. **Blocker — Q1–Q2: neither the proposed step joint nor the optional +8 brackets has a demonstrated fall-load capacity.**  
   `data/torso-stepped-README.md:49` substitutes perfectly bonded **20 × 20 × 2 mm tubes** for slotted extrusion and real joints. That cannot establish T-nut slip, slot-lip failure, bracket rotation or load sharing between front and rear gussets.

   The **860 N** calculation at line 83 is \(4300/5\), but the released plate has **three upper-post, three lower-post and three shared-rail bolts**. Those groups transmit load between members; nine bolts do not automatically act in parallel. If the stated 4.3 kN enters through the upper-post group, its average resultant is already **1.43 kN per bolt**, before eccentricity. The along-slot component needs a separate check.

   Even the packet’s assumed **1 kN** slip resistance gives only **1.16×** margin over 860 N. The bearing arithmetic \(860/(5×3)=57.3\) MPa is correct, but does not check plate tear-out, bolt-group moments or extrusion lips.  
   **Smallest fix:** resolve the forces at each attachment group and check the selected nuts, profile, brackets and preload. Add the Z-face brackets if that check requires them; adding eight brackets alone does not prove the joint.

3. **Blocker — Q2: the supplied cutting file does not contain the specified R15.**  
   `data/gusset_plate_left.dxf:118–130` defines the outer step transition with an **8 mm radius**, centred at **(105, 337.695)**. `scripts/frame.py:95` instead requests **R15** at the nominal **(113,335)** corner.

   Furthermore, `scripts/viewer.js:33` draws quadratic curves, so the rendered corner is not evidence of a true circular fillet. No supplied result establishes that R15 removes the reported stress concentration. The bonded frame still reports **297 MPa** locally against its assumed **145 MPa** yield (`torso-stepped-README.md:49,65`); a percentile does not establish absence of local yielding.  
   **Smallest fix:** release one authoritative, dimensioned DXF with the intended fillets and holes, and check that geometry against an explicit allowable-damage criterion.

4. **Blocker — Q2–Q3: the specified nut geometry cannot fit the gusset hole spacing.**  
   `scripts/frame.py:157–164` models T-nuts **20 mm long**. Upper-post gusset holes are at Y **385, 400, 415** (`:96`): adjacent nuts overlap by **5 mm**. `data/parts.json` confirms those overlapping volumes. The shared-rail nut centred at X **104** also extends to **114**, beyond the rail end at **113**.

   The assumed pitch screws create another conflict: their centres at Y **390 and 470** (`:204`) are only **2.38 and 5 mm** from the corresponding corner-bracket post screws. Separate nuts cannot occupy those positions in the same slot.  
   **Smallest fix:** select actual nut dimensions and the actual pitch-bracket hole pattern, then reconcile all hole positions before cutting the plates.

5. **Major — Q3: the assembly sequence lacks a nut-loading plan and has an unresolved shelf collision.**  
   From `scripts/frame.py`, each shared rail consumes these slots:

   | Rail face | Occupants |
   |---|---|
   | Outer Z face | Gussets at X **±62, ±84, ±104**; H3 at **±35** |
   | Inner Z face | Shelf-rail and step-side-rail brackets, nominal screw centres **±41, ±79** |
   | Top | Upper-post corner-bracket feet |
   | Underside | Lower-post corner-bracket feet, after finding 1 is corrected |

   H3 slide-in nuts cannot pass already fastened gusset nuts. H1/H2, H4 and H6 also have rail-end access constrained by adjoining posts. The shell sequence—frame and actuators, harness, then hardpoints—therefore requires preloaded nuts or identified drop-in nuts (`body-shell-manufacturing-README.md:111`).

   Also, shelf bracket `br24` occupies X **27–55**, Y **353.62–373.62**, Z **46–50**, overlapping the shelf’s stated envelope X **±55**, Y **354.6–372.6**, Z **±50**. Actual shelf cut-outs might resolve this; none are supplied.  
   **Smallest fix:** document nut type, insertion stage and tool approach for every occupied slot. Check the real shelf against all four end brackets, and install electronics after structural fasteners are tightened.

6. **Major — Q4: the Jetson and exhaust-fan clearance claims omit necessary geometry.**  
   The lateral arithmetic is correct: \((156.8-103)/2=\mathbf{26.9}\) mm. The vertical claim is incomplete. `scripts/frame.py:262` reserves Y **410–445**, omitting the additional **10 mm** cooler allowance. Including that allowance leaves **20 mm**, not 30 mm, below the top rails at Y **475**.

   A full-height cooler envelope would overlap the exhaust-fan envelopes beginning at Y **448**; the actual cooler shape might avoid this, but it is absent. Separately, the exhaust fans extend to Y **488** at Z **−50 to −40** (`:264–265`). Their upper **13 mm** discharge directly toward the rear top rail, which occupies Y **475–495**, Z **−70 to −50**. Raising D1 does not remove that obstruction.  
   **Smallest fix:** include the cooler, shelf, standoffs, plugs and fan discharge paths in one assembly check; reposition or duct the exhausts.

7. **Major — Q4: rear-door serviceability is not established, and the pad envelopes encroach on both doors.**  
   D1 is variously **120 mm wide**, **124 mm wide**, Y **360–480**, or Y **360–492** (`PACKET.md`, `torso-frame-README.md:167`, shell manufacturing §5).

   More decisively, `scripts/frame.py:275–279` puts upper pads at \(|X|≥58\), leaving **116 mm** between them, versus D1’s claimed **124 mm** opening. Lower pads leave **108 mm**, versus D2’s **116 mm** opening. The envelopes intrude **4 mm per side**, before hinge sweep and fitting clearance.

   Using the stated upper-back gap of **10–17 mm**, Pi ports projecting **1.8 mm** leave only **8.2–15.2 mm** for the mating plug and cable turn. Bare-board fit does not establish plugged-board removal or screwdriver access beneath the Jetson shelf.  
   **Smallest fix:** freeze clear opening dimensions, move the pad inner edges accordingly, and demonstrate unplugging and removing both boards with the arms and adjacent panels fitted.

8. **Blocker — Q1–Q2, Q4: the crush pads do not establish the stopping distance or load path used to justify the frame.**  
   The fall arithmetic checks: \(40×9.81×0.75=\mathbf{294.3}\) J; 60% stopped over 40 mm with a 1.5 peak factor gives **6.62 kN**.

   But the back case already reaches **146 MPa** against **145 MPa** assumed yield (`torso-stepped-README.md:142–150`). Coupon C9 targets energy absorption within **25 mm**, while the pad proposal supplies only nominal geometry. Even granting another 10 mm of usable shell travel gives **35 mm**; the packet’s own scaling raises 146 MPa to approximately **167 MPa**.

   The pads load H2/H3/H4 through standoffs and printed brackets (`body-shell-manufacturing-README.md:188–190`), whereas the FEA fixes frame rails directly. Six rear hardpoints sharing 6.62 kN equally would average **1.10 kN each**, before unequal contact and moments. That path has no demonstrated capacity.  
   **Smallest fix:** establish the pads’ force–travel curve and the complete standoff-to-extrusion load path, then recheck the frame against the resulting reactions and usable stroke.

9. **Major — Q5: the short top brackets clear the wing nominally, but their strength is unverified.**  
   With the packet’s geometry, the 28 mm bracket reaches down from Y **475 to 447**, overlapping the wing ending at **450**. A 20 mm bracket stops at **455**, giving **5 mm** nominal clearance.

   That proves clearance only. With the generator’s centred holes, shortening the leg moves the screw lever arm from **14 to 10 mm**; a simple bracket-moment comparison raises screw force by **40%**. The bonded FEA models neither bracket.  
   **Smallest fix:** specify the actual short bracket, material, hole position and rating, then check its joint reactions. “Any 2020-series bracket” is insufficient.

10. **Blocker — Q6: the proposed screw lengths do not match the modelled attachment stack.**  
    `scripts/frame.py:206` places pitch-screw heads at X **78.4** and specifies **M5×12**. Their tips reach **90.4**, short of the post face at **93**, before entering any nut. A real counterbore could change this; it is not defined.

    For hardpoints, **M5×16 through 12 mm PETG** leaves **4 mm** beyond the extrusion face. With the modelled nut beginning **1.5 mm** below that face, maximum engagement is only **2.5 mm**, before washers and tolerances.  
    **Smallest fix:** dimension the actual head seats and fastening stacks; select lengths for adequate engagement without bottoming. Include preload retention through PETG.

11. **Major — Q6: the purchase quantities are conditional and omit mounting hardware.**  
    The nominal arithmetic **64+36+20+8=128** M5 fasteners checks. However:

    - Existing **32 + 100 purchased nuts =132**, leaving four spares. The optional eight brackets raise demand to **144**.
    - The shell document estimates **40** M5 hardpoint fasteners, while the frame counts **20** (`body-shell-manufacturing-README.md:108`).
    - Jetson shelf attachments, PDB supports, third-board mounts, fan mounts/ducts, pad standoffs and waist-adapter hardware are not closed out.

    **Smallest fix:** choose the hardpoint fastening arrangement and structural brackets, then issue a complete fastener schedule with nut variants, grades, washers, lengths and quantities.

12. **Major — Q6: the reusable-stock assumption can invalidate the cut list.**  
    `data/master-bom.csv` and `torso-2020-extrusion.md` list **12 extrusion pieces totalling 2200 mm**. Rev A assumes two additional existing 100 mm shelf rails: **14 pieces and 2400 mm**. If those rails do not exist, new-stock demand becomes **1224 mm**, rather than 1024 mm.

    “One full stick plus offcuts under 300 mm cannot supply the cuts” is not a valid deduction without measured offcut lengths. One 500 mm stick can provide both 226s; suitable offcuts can provide the remaining cuts.

    **Verified:** rev A totals **3124 mm**, including **1024 mm** nominal new material. Three fresh 500 mm sticks suffice, and four 58×140 plate blanks fit a 150×300 sheet with cutting allowance. The post lengths sum to **320 mm**; two saw cuts reduce the nominal 20 mm remainder by **two kerfs**.  
    **Smallest fix:** inventory the actual rails and offcuts, then issue a kerf-aware nesting plan and finished-length tolerances.

13. **Minor — Q4, Q6: several generated numbers and drawing labels are misleading.**  
    The supplied R8 DXF has approximately **4959.5 mm²** net plate area. Assuming aluminium density **2.70 g/cm³**, each 3 mm plate weighs **40.2 g**, so four weigh **160.7 g**, not 40 g (`frame.py:104`, BOM item 10).

    The lower free height is **353.62−175=178.62 mm**, not 198.6 mm. The PDB model is **24 mm deep**, versus its stated **28 mm** allowance (`frame.py:218`). Finally, `draw2d.py:15–40` projects whole parts without section-plane clipping; `FRAME_PACKAGING.png` therefore does not contain the labelled true sections.  
    **Smallest fix:** correct the masses and envelopes, and label projections accurately or generate actual sections.

14. **Blocker — Q7: the waist stack consumes pelvis space that has already been assigned to the battery.**  
    The quoted **17 mm** shortage excludes the adapter. `frame.py` places a **6 mm** plate at Y **149–155**, and the **57 mm** motor below it at Y **92–149**. Against the waist bay Y **115–155**, the actual intrusion is **23 mm**.

    Where their footprints overlap, a **90 mm-high** battery plus that intrusion consumes **113 mm** of a **115 mm** pelvis envelope—only **2 mm** remains for floor structure, mounting, insulation and clearance. A pocket in a 6 mm adapter cannot recover 17 mm.

    The adapter bolt pattern and waist bearing/load path are unresolved. It is therefore unsupported to claim that the hip-stage solution cannot change this frame.  
    **Smallest fix:** freeze a waist cross-section containing the real adapter, motor, battery, hip structure and cable exits while preserving the required shoulder position. Determine any frame-interface changes before recutting the posts.

**Extrusion: NO-GO. Gusset plates: NO-GO.**