**The packet is not ready for cutting.** It contains assembly collisions, an incompatible gusset/T-nut layout, and unresolved interfaces that could change the cuts.

I read the packet, all supplied data and scripts, and all eight views. **Verified** below means checked against those files by independent arithmetic or coordinate intersection—not against the physical assembly. Actual stock, vendor fastener geometry, friction, printed-part strength, and the reported CAD/FEA results remain unverified. Nothing was modified or operated.

1. **BLOCKER — The modeled step cannot assemble as drawn. [Q1, Q3]**

   **Verified:** [scripts/frame.py:127](J:/code/marengo/hardware/designs/torso-frame/review/packet/scripts/frame.py:127) selects the upper half-width whenever `y >= Y_STEP_LO`. The lower-post-top brackets are placed exactly at that boundary, so their mounting faces land at **X ±93 instead of ±55**, a 38 mm error. In `data/parts.json`, `br2/br8/br14/br20` each penetrate a lower post by **10 × 4 × 20 mm**, while their vertical legs miss its mounting face.

   Separately, the step-side and shelf rails leave only **93 − 75 = 18 mm** between them. The 28 mm brackets generated at [frame.py:143](J:/code/marengo/hardware/designs/torso-frame/review/packet/scripts/frame.py:143) intrude **10 mm** into the shelf rails. All four do this.

   The four shelf-end brackets also intersect the modeled shelf plate: each leg overlaps its envelope by **28 × 18 × 4 mm**. Actual shelf reliefs might resolve that last conflict, but none are represented.

   **Smallest fix:** correct the lower bracket coordinates; replace or relocate the four step-side connections so they fit the 18 mm gap; explicitly model shelf reliefs or alternative connections. Recheck these joints before deciding whether the optional eight brackets fit.

2. **BLOCKER — The gusset cutting file and specified nuts are incompatible with the release. [Q2, Q3]**

   **Verified:** [frame.py:96](J:/code/marengo/hardware/designs/torso-frame/review/packet/scripts/frame.py:96) places upper-post holes at **Y 385, 400, 415**, giving 15 mm pitch. Its T-nuts are **20 mm long** along the slot ([line 158](J:/code/marengo/hardware/designs/torso-frame/review/packet/scripts/frame.py:158)). The generated nuts overlap adjacent nuts by **5 mm**. A different, shorter purchased nut could work; the modeled one cannot.

   Also, [gusset_plate_left.dxf](J:/code/marengo/hardware/designs/torso-frame/review/packet/data/gusset_plate_left.dxf) contains **seven R8 arcs**, including the corner associated with X113/Y335. It does not contain the specified R15 revision. The viewer uses quadratic curves rather than exact circular fillets, so its appearance does not verify the cutting geometry.

   **Smallest fix:** select the actual T-nut first, then release one dimensioned gusset drawing and matching DXF with compatible hole spacing, end clearance, and the intended circular radii. Do not send the current DXF for cutting.

3. **BLOCKER — Neither the base step joint nor “optional +8” has demonstrated capacity after slip. [Q1, Q2]**

   **Verified arithmetic:** [torso-stepped-README.md:83](J:/code/marengo/hardware/designs/torso-frame/review/packet/data/torso-stepped-README.md:83) gives:

   - \(4300/5 = 860\) N per bolt.
   - Nominal plate bearing \(860/(5×3)=57.3\) MPa.
   - Assumed friction margin \(1000/860=1.16\).

   Those calculations are arithmetically correct, but the **five-bolt load-sharing assumption is unexplained**. The actual plate has three bolts into each post and three into the rail. If a three-bolt post connection transfers the stated 4.3 kN share, its average is **1.43 kN per bolt**, before eccentricity—already above the assumed 1 kN friction capacity. Shared-rail participation must be calculated, not presumed.

   Bolt bearing in the plate also does not prevent a T-nut sliding **along the extrusion slot**. Plate bearing, nut slip, slot-lip failure, bracket prying, and joint separation are different failure modes.

   The FEA uses perfectly bonded substitute tubes ([line 49](J:/code/marengo/hardware/designs/torso-frame/review/packet/data/torso-stepped-README.md:49)); it therefore cannot decide whether one bracket per post end is enough. Nor does changing a plate radius establish that the reported **297 MPa frame peak** is resolved.

   **Smallest fix:** calculate the actual bolt-group reactions and qualify the selected extrusion/nut/bracket connection at a specified preload, including residual load paths after slip. Add a positive restraint or revise the connections if needed. **Neither the existing arrangement nor the optional eight brackets can presently be accepted as sufficient. R15 is unproven.**

4. **BLOCKER — The rear-fall survival claim depends on unavailable crush travel and an undesigned mounting path. [Q1, Q7]**

   **Verified:** the stated fall assumptions give:

   \[
   E=40×9.81×0.75=294.3\text{ J},\quad
   E_\text{contact}=176.6\text{ J},\quad
   F_\text{peak}=1.5E_\text{contact}/0.040=6.62\text{ kN}.
   \]

   The back-case result is **146 MPa against a 145 MPa limit at 40 mm travel** ([torso-stepped-README.md:142](J:/code/marengo/hardware/designs/torso-frame/review/packet/data/torso-stepped-README.md:142)). Even its own linear model needs about **40.3 mm**, with no margin.

   A pad standing 30 mm proud does not provide 30 mm usable crush. The shell coupon target is only **25 mm** ([body-shell-manufacturing-README.md:182](J:/code/marengo/hardware/designs/torso-frame/review/packet/data/body-shell-manufacturing-README.md:182)); adding the quoted 8–17 mm gap gives **33–42 mm geometrically**, before accounting for rigid standoffs or other contact. That gap cannot automatically be counted as energy-absorbing travel.

   The pad loads are routed into placeholder PETG hardpoints. Even ideal sharing across all six rear H2/H3/H4 attachments gives **1.10 kN each**; first contact may involve fewer.

   **Smallest fix:** define usable force–displacement travel and a structural pad-to-frame load path, then reassess the frame with that load. Withdraw the current fall-survival claim until this is resolved.

5. **BLOCKER — The waist deficit is 23 mm in this model, and it competes directly with the pelvis battery. [Q7]**

   **Verified:** [frame.py:229](J:/code/marengo/hardware/designs/torso-frame/review/packet/scripts/frame.py:229) places the adapter at **Y149–155** and the 57 mm motor at **Y92–149**. The reserved bay begins at Y115:

   \[
   57+6-40=\mathbf{23\text{ mm}}.
   \]

   The repeatedly quoted **17 mm** excludes the adapter.

   The draft pelvis is 115 mm tall and the battery 90 mm tall ([torso-design-handoff.md](J:/code/marengo/hardware/designs/torso-frame/review/packet/data/torso-design-handoff.md), “Draft: pelvis & power”). A 23 mm intrusion leaves **92 mm** beneath it—only **2 mm** beyond the bare battery height, before structure, restraint, insulation, or clearance. This is a conditional stack conflict, since the battery position is not modeled.

   The adapter bolt pattern and waist bearing moment capacity also remain unresolved. Therefore “hip-stage decision, does not change this frame” is unsupported.

   **Smallest fix:** freeze the adapter, waist motor, bearing/load path, battery position, and cable sweep together. Pocketing or translating the motor may preserve the frame cuts, but that must be demonstrated before shortening the existing posts.

6. **MAJOR — The assumed shoulder fasteners cannot reach their intended mounting faces. [Q3, Q5, Q6]**

   **Verified:** the M5×12 shoulder screws start at **X78.4** and end at **X90.4**, while the post mounting face is **X93** ([frame.py:206](J:/code/marengo/hardware/designs/torso-frame/review/packet/scripts/frame.py:206)). They stop **2.6 mm short of the extrusion**, before any thread engagement. A real recessed screw seat could change this, but none is defined.

   Their assumed heights, **Y390 and Y470**, also fall within the lower and upper inner-X corner-bracket footprints. The pitch bracket and corner brackets cannot independently claim those mounting locations.

   For hardpoints, **16 − 12 = 4 mm** is all that remains beyond the PETG. Using the model’s 1.5 mm nut setback leaves at most **2.5 mm engagement**, before washers or thread chamfers.

   **Smallest fix:** measure the existing pitch-bracket holes and seating depths, resolve shared mounting space, and specify screw lengths from the actual grip and nut geometry. Use defined compression sleeves or another preload-preserving detail where structural clamping passes through PETG.

7. **MAJOR — The packet supplies a shell sequence, not a buildable frame/T-nut sequence. [Q3]**

   After correcting finding 1, the shared rails need this slot allocation:

   | Shared-rail face | Occupants |
   |---|---|
   | Top, +Y | Upper-post inner-X brackets |
   | Bottom, −Y | Lower-post inner-X brackets |
   | Inward Z face | Shelf-rail and step-side-rail end connections |
   | Outward Z face | Gusset nuts at X ±62, ±84, ±104; H3 at X ±35 |

   This follows [frame.py:124](J:/code/marengo/hardware/designs/torso-frame/review/packet/scripts/frame.py:124) and the gusset/hardpoint coordinates. Posts do **not** automatically close every shared-rail end, but installed nuts, brackets, actuators, and shelf parts can obstruct insertion routes and tool access.

   The shell sequence installs hardpoints **after the harness** ([body-shell-manufacturing-README.md:111](J:/code/marengo/hardware/designs/torso-frame/review/packet/data/body-shell-manufacturing-README.md:111)); no late-insertion nut type is specified.

   **Smallest fix:** preload structural slide-in nuts before closing the relevant members; assemble and torque the inner connections before the shelf and actuators obstruct them. Identify exact compatible drop-in nuts for late hardpoints—especially H3—and any late gusset installation. Show driver access and a removal sequence. The optional eight brackets need their own slot allocation.

8. **MAJOR — The cooling arrangement does not have the claimed clearance. [Q4]**

   **Verified:** the Jetson model stops at Y445 and omits its separately specified **10 mm cooler** ([frame.py:262](J:/code/marengo/hardware/designs/torso-frame/review/packet/scripts/frame.py:262)). Including it leaves:

   \[
   475-(410+35+10)=\mathbf{20\text{ mm}},
   \]

   not 30 mm.

   The exhaust fans occupy **Y448–488, Z−50…−40**. The rear top rail occupies **Y475–495, Z−70…−50**. Consequently, **13 mm of each 40 mm fan height—32.5% of its square face—blows directly toward the rail**, unless a duct redirects it. This is airflow obstruction, even though the solids only touch at Z−50.

   Moving the fans straight down below the rail would put them into the Jetson’s current envelope.

   **Smallest fix:** model the actual cooler, shelf supports, fans, guards, and exhaust ducts together. Relocate or duct the fans and verify clearance plus delivered airflow through the assembled shell.

9. **MAJOR — Bare-board fit does not establish service through D1. [Q4]**

   **Verified:** the Jetson’s lateral allowance is **(156.8−103)/2 = 26.9 mm**, but its depth allowance inside the nominal bay is only **100−90.5 = 9.5 mm total**. Its current rear edge is 24.5 mm ahead of the frame’s rear face; neither plugs nor cable bends are modeled.

   Pi ports project **1.8 mm** behind that face. Against the reported 10–17 mm upper-back gap, that leaves only **8.2–15.2 mm** to the shell before connector accommodation.

   The door dimensions disagree: the frame README gives **Y360–480**, while the shell document gives **Y360–492**, 124 mm wide. More seriously, generated upper pads start at **|X|58**, overlapping that door’s **|X|<62** region by **4 mm per side**; the lower pads similarly overlap the stated D2 region. See [frame.py:275](J:/code/marengo/hardware/designs/torso-frame/review/packet/scripts/frame.py:275).

   **Smallest fix:** reconcile the door and pad geometry; model the installed connectors and removal sweeps. Demonstrate unplugging and removing each board with the other installed. The packet proves neither serviceability nor impossibility of servicing after redesign.

10. **MAJOR — The four shorter top brackets have not been assessed under the actual arm load. [Q5]**

    The bonded FEA omits both bracket variants, so its low bulk-frame stresses cannot establish their relative strength or stiffness.

    **Calculated illustration:** reacting the specified **60 N·m** pitch moment across the **120 mm front/rear post-center spacing** requires a **500 N force couple**. With symmetric sharing of the stated 330 N downward load, the two post reactions would be approximately **+665 N and −335 N**, before other load components. The connection must therefore resist uplift as well as compression.

    A shorter bracket can change prying and screw leverage; it is not valid to infer capacity from leg length alone. “Any 2020-series inside corner bracket” in the shopping list is insufficient.

    **Smallest fix:** specify the exact bracket and hole geometry, then check its connection loads with the real pitch mount. The recorded wing clearance supports fit only, not capacity.

11. **MAJOR — The procurement plan is not derived from a verified inventory. [Q6]**

    **Verified arithmetic:** total extrusion **3124 mm**, nominal new material **1024 mm**, and post cuts **198.62 + 121.38 = 320 mm** are correct. The 20 mm remainder must also absorb kerf and facing; it will not remain a 20 mm offcut.

    However:

    - `master-bom.csv` lists **four 100 mm rails**; rev A assumes **six existing**.
    - Its listed original members total **2200 mm**; adding two shelf rails gives **2400 mm**, not the **2460 mm** in the stepped-frame study.
    - “Offcuts under 300 mm cannot yield the required rails” is false. For example, with 3 mm kerfs, one 500 mm stick can yield two 226s; each of two 294 mm offcuts can yield **186 + 100 + 6 = 292 mm**.
    - The two discarded 110 mm top rails could instead become the two 100 mm shelf rails, reducing nominal new-stock demand from **1024 to 824 mm**, conditional on inventory.
    - The shell plan estimates **40 hardpoint M5 fasteners**; the frame counts **20**. If two per hardpoint are required, total M5 demand becomes **148**, and 32 existing plus 100 purchased nuts is insufficient.

    **Smallest fix:** inventory and assign every retained member/offcut; resolve hardpoint fastening before ordering. The 150×300 sheet does accommodate four 58×140 blanks arranged **116×280 mm**, but only the corrected gusset drawing should be cut. Add the final shelf, fan/duct, pad-standoff, and adapter hardware to the BOM.

12. **MAJOR — The packaging evidence omits obstacles and changes component envelopes. [Q4]**

    **Verified:** [draw2d.py:15](J:/code/marengo/hardware/designs/torso-frame/review/packet/scripts/draw2d.py:15) projects entire parts; it does not perform the labeled section cuts. It skips all multibox corner brackets and excludes hardpoints and fasteners from its drawing groups. Thus `FRAME_PACKAGING.png` cannot establish unobstructed internal space.

    Additional discrepancies in `frame.py`:

    - PDB depth modeled as **24 mm**, labeled **28 mm**.
    - “90×60×20” third board modeled as **90×90×20**.
    - “40×40×10” intake fan modeled as **40×45×10**.
    - The two Ø10 harness trunks at **X±45, Z40** intersect the third-board envelope, which reaches **X±45, Z38**.

    **Smallest fix:** reconcile envelopes, route the harness clear, and perform interference and access checks including brackets, fasteners, supports, and connectors. Label projections accurately.

13. **MINOR — Mass and usable-height errors contaminate downstream sizing. [Q6, Q7]**

    **Verified calculation, assuming aluminium density 2.70 g/cm³:** the R8 DXF outline minus nine Ø5.5 holes has approximately **4959.5 mm²** net area. At 3 mm thickness that is **40.2 g per plate**, or **160.7 g total**, not the BOM’s **40 g total**. R15 changes this negligibly.

    The lower usable height above the bottom rails is **353.62−175 = 178.62 mm**, not the 198.6 mm stated in the frame README’s packaging paragraph. The packaging drawing uses the correct shorter value.

    **Smallest fix:** correct these quantities and reconcile hardware mass before reusing the torso mass estimate for hip, balance, or impact calculations.

**NO-GO for cutting the extrusion. NO-GO for cutting the gusset plates.**