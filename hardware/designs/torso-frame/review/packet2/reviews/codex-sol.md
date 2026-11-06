**NO-GO on cutting the posts or ordering the custom gussets.** The packet contains actual envelope conflicts, an invalid fastener-placement model, and unresolved crash retention and thermal requirements that could change the packaging and joint design.

I read the packet, supporting documents, all three scripts, the DXF, JSON files, drawings and five photographs. I screened the 526 `parts.json` entries with independent, in-memory calculations; I did not run the generators or touch hardware.

“Verified” below means verified from the supplied geometry or arithmetic. It does **not** mean verified on the physical assembly. Actual print geometry, tolerances, cable bends, connector holders, mounting cavities and impact behaviour remain unverified.

**1. Pack retention — BLOCKER**

At the stated 17 g, the pack imposes:

\[
F=2.2(17)(9.81)=367\ \mathrm N
\]

That is the nominal inertial load in **each possible fall direction**, before allowance for uneven load sharing, clearance take-up or a shorter stopping distance.

The model does not provide a complete restraint system:

| Pack motion | What the packet provides | Failure |
|---|---|---|
| Forward, +Z | Side rails; carrier begins at Z 37.5 | Only 3.5 mm separates the pack face at Z 34 from the carrier. The boards become the stop. |
| Rearward, −Z | D2 door, magnets and an M3 run-lock | Neither magnets nor the door/hinge/lock assembly has a demonstrated 367 N load path. |
| Sideways, ±X | Printed rails at \|X\| 47.5–55 | Rail attachment, print orientation and bearing strength are unspecified. |
| Upward, +Y | Pack top Y 345; shared rail underside Y 353.62 | The pack can move into its power/sense harness before meeting structural restraint. |
| Downward, −Y | Described as a tray | The generated rails are side blocks, without a represented supporting ledge beneath the pack. |

Evidence: `data/parts.json`, IDs `pack`, `packrail_±1`, `pdb_carrier`, `harness_pack`; [frame.py:293–299](/Users/joseph/scratch/frame-gate2/hardware/designs/torso-frame/review/packet2/scripts/frame.py:293). D2 construction is described in [shell_manufacturing_README.md:119–123](/Users/joseph/scratch/frame-gate2/hardware/designs/torso-frame/review/packet2/data/shell_manufacturing_README.md:119).

**Smallest fix:** keep the printed rails as guides, but add a frame-attached cradle with a bottom support, forward stop, top restraint and a removable rear retaining bar or positive latch. Restraint should bear on the pack case, not the cells, sense board or connectors. Size the attachments for the crash load with an explicit margin. The D2 door should remain a cover, independent of pack retention.

Adding a stronger door lock alone does not solve forward or upward restraint.

**2. Electrical-mechanical safety — BLOCKER**

The 3 mm air gap and 0.5 mm Nomex are not a crash barrier. The pack needs to move only 3.5 mm forward to reach the carrier. A distorted carrier, loose terminal screw or displaced lug can defeat the nominal separation.

Specific exposure points are:

- Pack power and sense wiring over Y 345–353.6 can be trapped beneath the shelf or bracket edges.
- The front trunks already intersect aluminium rails and bracket hardware; see question 3.
- The side routing reservation is only **5 × 10 mm per side**, at \|X\| 50–55, Z 40–50. It is not a demonstrated bundle envelope for waist, auxiliary, charge and future leg conductors, including insulation, terminals and strain relief.
- Ring terminals remain exposed conductors. “No plugs” removes connector bulk but does not establish clearance around lug barrels, screw heads or cable bends.
- The removed pack remains electrically live upstream of the remote BMS. Its fuse, connector holder and terminal protection are not represented in `parts.json`.

Evidence: [pdb_design_README.md:21–48](/Users/joseph/scratch/frame-gate2/hardware/designs/torso-frame/review/packet2/data/pdb_design_README.md:21), `data/parts.json` IDs `harness_pack`, `harness_down_±1`, `harness_side_±1`, and `pdb_sheet`.

There is also a **verified mounting-stack inconsistency**:

- Carrier: Z 37.5–41.5.
- Board envelope starts: Z 43.5.
- Available separation: **2 mm**, although the specification says **6 mm standoffs**.
- Keeping the represented 18 mm board/component envelope with actual 6 mm standoffs moves its front to **Z 65.5**, before screw heads and wiring.
- The drawn M3×8 shafts occupy Z 53.5–61.5 and do not reach the carrier.

This is a model error, not proof that an actual M3 screw must pass through the entire component envelope. It means the claimed 24 mm assembly depth has not been established.

The **1 MΩ ∥ 10 nF connection is an ESD/reference connection, not a fault-clearing bond**. A positive-to-frame fault would pass only approximately:

\[
54.6/1\,000\,000=55\ \mu\mathrm A
\]

through the resistor. That will not clear the pack fuse. The frame can remain near pack-positive potential until another contact creates a return path, potentially through a cable shield, peripheral or mounting fastener. The capacitor contributes transient current; it does not provide a sustained clearing path.

The network does not inherently create the short. Treating it as protective bonding makes the safety argument worse. Contact with actuator housings also does not establish electrical continuity through the complete painted/anodized/printed assembly.

**Smallest fix:** first provide positive pack restraint, captive terminal covers, fixed strain reliefs and edge protection. Then define and verify the actual PCB/standoff stack and grounding/fault-clearing scheme. The insulation sheet needs retained coverage under displacement; an unrestrained flat sheet is insufficient.

**3. Interference — the “free of interference” claim is false**

All coordinates below are millimetres. Evidence is [parts.json](/Users/joseph/scratch/frame-gate2/hardware/designs/torso-frame/review/packet2/data/parts.json); the generating locations are in `scripts/frame.py`. Mirrored cases are grouped, with their IDs explicitly identified.

I excluded intended fastening engagement, normal member joints, pack-to-guide contact, and the explicitly unplaced `future` envelopes. Filled actuator/bracket envelopes require separate treatment because their real mounting cavities are absent.

The most consequential error is that **the shared front rail occupies Z 50–70, not Z 60–80 as stated in the connector-room discussion**. The shelf reaches Z 50. Therefore the proposed Z 50–60 cable passage is occupied by aluminium.

| Rank | Parts | Verified conflict and coordinates | Smallest fix |
|---|---|---|---|
| **Blocker** | `harness_step` / `rail_shared_1` | Full intersection: X 33–45, Y 353.62–373.62, Z 50–60. | Route around the rail; the claimed internal passage does not exist. |
| **Major** | `harness_down_±1` / `rail_bot_1`, `rail_shared_1` | Trunks at X ±51, Z 66, radius 4 pass through the bottom rail at Y 160–175 and shared rail at Y 353.62–370. | Redraw continuous routes around both rails, including bends and clips. |
| **Major** | `harness_down_1` / `br0`, `br2`; `harness_down_-1` / `br12`, `br14` | Intersect the front bracket legs over Y 175–203 and 325.62–353.62, around \|X\| 51–55, Z 62–70. | Reserve a route outside the bracket and fastener envelopes. |
| **Major** | `harness_shoulder_±1` / `jetson` | Overlap at \|X\| 42–51.5, Y 447–455, Z −32 to −22. This is a cylinder/box intersection, not just overlapping bounding boxes. | Raise or redirect the shoulder runs, then check the connector bend. |
| **Major** | `shelf` / `br24`, `br26`, `br28`, `br30` | Bracket legs occupy the shelf at Y 354.6–372.6, along \|X\| 51–55 and Z ±46–50. | Provide actual shelf reliefs or relocate its brackets. “Shelf unchanged” is not demonstrated. |
| **Major** | `pdb_carrier` / `packrail_±1` | Each overlap is 2.5 mm in X and Z: \|X\| 47.5–50, Y 190–340, Z 37.5–40. | Relieve the rail/carrier interface and recheck its attachments. |
| **Major** | `spine_rail` / `gusset_1_-1`, `gusset_-1_-1` | Actual plate/lip intersection near \|X\| 55–60, Y 355–365, Z −73 to −70. | Narrow or notch the lip, allowing clearance for the gusset screws too. |
| **Major** | `pitch_wing_1` / `br3`, `br9`; `pitch_wing_-1` / `br15`, `br21` | Envelope intersection at \|X\| 89–93, Y 400–401.62, \|Z\| 50–69. | Check native geometry; shorten/reposition these four brackets if the wing occupies that region. |
| **Major** | `pdb_sheet` / `packrail_±1` | Sheet envelope intersects the rails at \|X\| 47.5–50, Y 190–340, Z 34–34.5. | Shape the sheet/rails while retaining full electrical coverage. The sheet may flex, but the represented solids do not fit. |

These additional non-mating clearances are below 2 mm:

| Rank | Parts | Verified minimum clearance/location | Smallest fix |
|---|---|---|---|
| **Major** | `pdb`, `pdb_logic` / front brackets `br2`, `br14` and `br0`, `br12`, respectively | **1 mm** laterally, board edge X ±50 against bracket face X ±51; Y 325.62–340 or 190–203. | Include tolerances and terminal/PCB keep-outs; create more separation. |
| **Major** | `pdb`, `pdb_logic` / `harness_down_±1` | **0.610 mm**, near the board front corners, Z 61.5. | Move trunks away from components and mounting screws. |
| **Major** | `pitch_wing_±1` / corresponding front/rear `gusset` | **1 mm**, wing Z ±69 versus plate inner face Z ±70, Y 400–425. | Establish tolerance and deflection clearance in native CAD. |
| **Major** | `rs03_pitch_±1` / `rail_stepside_±1` | **1.74 mm** vertically: motor bottom Y 375.36 versus rail top 373.62. | Check tolerances and deformed geometry. |
| **Major** | `rs03_pitch_±1` / `rail_topside_±1` | **0.64 mm** vertically: motor top Y 474.36 versus rail underside 475. | Same. |
| **Major** | `rs03_pitch_±1` / corresponding `post_up_*` | **0.5 mm** to front and rear posts at Z ±50. Already acknowledged, but not an adequate tolerance/impact assessment. | Verify actual motor/profile geometry and permissible contact. |
| **Major** | `rs03_pitch_1` / `br5`, `br11`; mirrored motor / `br17`, `br23` | **1.551 mm** at the bracket legs beginning Y 447, Z ±46. | Check actual bracket shape and movement clearance. |
| **Minor** | `harness_shoulder_±1` / corresponding `pitch_bracket`, `pitch_wing`, `rs03_boss` envelopes | **0.4 mm** at \|X\| 78–78.4. Connector flats are not modelled. | Replace the cylinder-only exit with the actual plug/lead envelope. |
| **Minor** | `harness_pack` / `shelf` | **1 mm**, harness top Y 353.6 versus shelf underside 354.6. | Provide controlled cable retention and clearance. |
| **Minor** | `harness_pack` / `br26`, `br30` | Approximately **1.000 mm**, at rear bracket tips near \|X\| 27–35, Z −45 to −22. | Redirect or protect the bundle. |
| **Minor** | `shelf` / four `post_lo_*` and `br2`, `br8`, `br14`, `br20` | **0.98 mm** at the shelf’s lower corners. | Include real reliefs and assembly tolerances. |
| **Minor** | `pi_stub` / `shelf` | **1.02 mm** vertically. Its actual mounting is absent. | Model the support/standoff rather than leaving an unexplained gap. |
| **Minor** | `H3_0…3` / corresponding `br2`, `br14`, `br8`, `br20` | **0.38 mm** vertically, Y 353.62–354. | Adjust the bracket footprint or establish controlled contact. |
| **Minor** | `spine_rail` / rear lower posts and `br8`, `br20` | **1.38 mm** vertically. | Trim the lip as part of resolving its gusset interference. |
| **Minor** | `packrail_1` / `br1`; mirrored rail / `br13` | **1 mm**, between bracket top Y 179 and guide bottom Y 180. | Include print/assembly tolerances. |

There are also zero-margin cable contacts: `harness_down_±1` is tangent to the front posts, gussets and H4 brackets, and comes within **0.123 mm** of H3. `harness_side_±1` touches the posts, bottom rail edge, tray rails and carrier/PCB edges. These are not all solid penetrations, but they leave no represented allowance for clips, sleeves or movement.

**Fasteners introduce further conflicts, and their model is internally wrong.**

For example, `br0` has its horizontal leg at Y 175–179, but `f0_0_h` occupies Y 175–179.5: the head is embedded in the bracket. Its shank starts at Y 175 instead of passing through the leg from Y 179. `t0_0` extends to Y 177.5, also into the bracket. The same seating error applies throughout `bolt_bracket()`.

Evidence: [frame.py:168–183](/Users/joseph/scratch/frame-gate2/hardware/designs/torso-frame/review/packet2/scripts/frame.py:168).

The additional non-mating fastener conflict families are:

| Rank | IDs/family | Coordinates and result | Smallest fix |
|---|---|---|---|
| **Major** | `f{24,26,28,30}_{0,1}_h`, corresponding `t*`, and shank boundary contacts / `shelf` | Screw heads and nuts occupy the shelf near \|X\| 36–55, Y 358.62–368.62, Z ±31–50. | Correct seating and provide shelf/bracket reliefs. |
| **Major** | `f{0,2,12,14}_1_h`, corresponding nuts / `harness_down_±1` | Intersections around Y 189 and 339.62, \|X\| 50.5–55, Z 62–65. Corresponding shanks are **1.315 mm** away. `t{0,2,12,14}_0` are **1.099 mm** away. | Correct seating, then reroute trunks. |
| **Major** | `pdb_m3_0…5_h` / front trunks | All six heads intersect the trunks at X ±46, Y 194/265/336, Z 61.5–63.2. Their shanks are approximately **1.701 mm** away. | Define actual PCB screw planes and move the trunks. |
| **Major** | `f2_0_*`, `t2_0`, `f24_0_*`, `t24_0` / `harness_step` | Intersections around X 36–46, Y 349.12–368.62, Z 50–60. | Eliminate the occupied cable passage. |
| **Major** | Rear `ggusset_{±1}_-1_3_h/s` / `spine_rail` | Intersect near \|X\| 57.75–60, Y 359.37–367.87, Z −77.5 to −70. Corresponding T-nuts are **1.5 mm** away. | Notch/narrow the lip around the complete fastening envelope. |
| **Major** | Rear gusset heads, hole indices 6–8 / `spine_upper_±1` | **0.5 mm penetration** into the pad envelope at \|X\| ≈103, Y 385/400/415, Z −77.5 to −77. Heads at indices 4–5 have only **0.13 mm** clearance to pad bottom Y 368. | Add intentional pad/backing reliefs. |
| **Major, model-dependent** | Crossed bracket shanks | `f{0,6,12,18}_1_s` cross `f{1,7,13,19}_1_s`; `f{4,10,16,22}_1_s` cross `f{5,11,17,23}_1_s`. Located around post centres \|X\| 65 or 103, \|Z\| 60. | Correct the seating error before deciding actual screw lengths/locations. |
| **Major, model-dependent** | Shared-rail bracket shanks | `f{2,8,14,20}_0_s` cross `f{24,26,28,30}_0_s`; `f{3,9,15,21}_0_s` cross `f{25,27,29,31}_0_s`, around Y 363.62. | Same; include a real slot-by-slot fastening map. |
| **Major, model-dependent** | Bracket / shell screws | `f{0,6,12,18}_0_s` cross the corresponding `hf_H4_*_s`; `f{1,7,13,19}_0_s` cross `hf_H6_*_s`, near Y 165. | Correct seating and check selected lengths. |
| **Minor, model-dependent** | Lower-step bracket / H3 screws | `f{2,8,14,20}_0_s` to corresponding `hf_H3_*_s`: **1.548 mm**, near X ±35–41, Y 364, Z ±60. | Resolve in the corrected fastening map. |
| **Major, model-dependent** | Bracket / gusset fasteners | Lower-post bracket indices `{2,8,14,20}`, leg 1, approach gusset hole 2; upper-post `{3,9,15,21}`, leg 1, approach hole 6; their leg 0 approaches shared-rail hole 4. Shank/nut contacts occur, shank/shank gaps are **0.502 or 1.405 mm**, nut/nut gaps **1.5 mm**. Coordinates: (\|X\|65,Y337), (103,385), (84,363.62), Z ≈±63–65. | Correct seating, then separate conflicting fasteners. |
| **Minor** | Two heads on short brackets `{4,10,16,22,25,27,29,31}` | **1.768 mm** head-to-head clearance. | Check actual head dimensions and tool access. |
| **Minor, model-dependent** | Front bracket heads / PCB screw heads | `f0_1_h`/`pdb_m3_0_h` and mirror: **1.859 mm**; `f2_1_h`/`pdb_m3_4_h` and mirror: **1.75 mm**. | Resolve after both mounting stacks are corrected. |

These numerical conflicts must not be presented as confirmed physical screw collisions until the placement error is fixed.

Finally, `pitch_bracket_±1` are filled boxes occupying the motor and portions of `br5/11/17/23` and their fasteners. Those are **incomplete representations of mounting cavities**, not evidence that the real parts collide. Conversely, they cannot substantiate a clean interference check.

**4. Serviceability — MAJOR: pack removal is plausible, but not demonstrated as a complete service operation**

The represented pack and sense board pass through the nominal opening:

- D2: 120 × 177.
- Pack: 95 × 162.
- Nominal opening margins: **12.5 mm each side, 8 mm below, 7 mm above**.
- Rear frame throat: 110 mm between posts, giving **7.5 mm each side** before guides and hardware.

My rearward swept-envelope check found no blocking intersection for the **pack and sense-board boxes**, excluding intended guide contact. That does not include the fuse, XT90S, mated sense connector, holder, pull handle, retention or strain reliefs.

The documented “pull ~90 mm” is insufficient to clear the frame. The pack’s forward face starts at Z +34; clearing the rear frame face at Z −70 requires **more than 104 mm translation**, and the shell opening requires more again.

A workable service sequence would be:

1. Power down, isolate external charging and allow stored bus energy to discharge.
2. Remove the D2 run-lock and open the door fully clear of the extraction envelope.
3. Unplug XT90 first, then sense connector, following the packet’s connection order.
4. Secure the board-side leads outside the extraction path.
5. Release a pack retainer independent of the door.
6. Pull the pack straight rearward through the complete opening.

Steps 2, 4 and 5 do not yet have adequate geometry. Connector accessibility cannot be verified from the sense-board box alone.

The boards require **U3 removal**, including its fan connections. They are not serviced through D2. After U3 removal, the intended sequence is to disconnect and secure pigtails, disconnect the HP/LP interconnect, remove the six PCB screws and withdraw the boards forward. However:

- Carrier-to-frame attachments are absent.
- The mounting stack is inconsistent.
- Terminal covers, connector withdrawal distances and tool access are absent.
- Carrier removal is obstructed by its tray-rail intersections.

**Smallest fix:** model the complete removable assemblies and their withdrawal/tool envelopes. Demonstrate both sequences before printing final carriers or trays.

**5. Cooling — MAJOR; BLOCKER for the claimed sustained-current packaging**

A path through the surrounding shell cavity is physically possible. The packet does not demonstrate the claimed internal chimney.

The shelf fills X ±55, Z ±50 at Y 354.6–372.6. It closes the central lower-to-upper passage. The proposed front cable/air gap is actually a rail. Air must detour outside the cage or through deliberately provided openings.

Above it, the Jetson leaves approximately:

- **26.9 mm** on each side;
- **4.5 mm** behind its envelope;
- **5 mm** in front.

Its second shelf, supports, connected ports and cable bends are absent. Rear exhausts can preferentially draw nearby upper-cavity air instead of pulling air across the lower boards and pack.

The largest thermal error is the pack:

\[
R_{\rm cells}=13(12.8\,{\rm m}\Omega)/2=83.2\,{\rm m}\Omega
\]

| Pack RMS current | Estimated cell heating |
|---:|---:|
| 10 A | 8.3 W |
| 20 A | 33.3 W |
| 40 A | **133 W** |
| 60 A | 300 W |

This uses the manufacturer’s typical DC resistance, not a measured pack resistance. Interconnects add losses; temperature and state of charge change the result. Nevertheless, **“a few W at 40 A” is incompatible with the packet’s own resistance basis**. The pack alone can exceed the shell’s entire 60–75 W heat budget. [Molicel P50B datasheet](https://www.molicel.com/wp-content/uploads/Product-Data-Sheet-of-INR-21700-P50B-80122.pdf)

**Worst likely sustained hot spot:** the interior cells of the enclosed pack, bounded by its case, guides and board carrier. Under repetitive braking, the resistor and adjacent printed guide become another critical location.

The resistor specification is also inconsistent. The model reserves **25 × 50 × 8**, but the proposed Arcol HS50 is approximately **28 × 72.5 × 14.8 mm**, and its 50 W rating requires specified heatsinking. It is rated only 14 W without that heatsink. A slotted rail is not automatically the required heatsink. [ARCOL manufacturer datasheet](https://www.mouser.com/datasheet/2/303/HS_Datasheet3-779273.pdf)

At the modelled location—X −55 to −47, Y 155–180, Z ±25—the resistor already touches the tray-rail envelope at Y 180. Substituting the proposed larger part requires a new clearance check. It is also below and lateral to the lower intake, not directly in its jet.

Evidence: `data/shell_manufacturing_README.md:133`, `data/pdb_PLAN.md:72–76`, `data/pdb_design_README.md:67–93`, and `parts.json` ID `brake_res`.

**Smallest fix:** establish the pack’s permitted RMS-current/duty envelope; select the actual resistor and heatsink; provide a deliberate lower-to-upper airflow route or separate lower exhaust. Validate temperatures in the closed configuration. Free-air fan ratings do not establish installed flow.

**6. Build and shopping list — geometry arithmetic checks; assembly and purchasing list do not**

Verified arithmetic:

- Lower post: 353.62 − 155 = **198.62**.
- Upper post: 495 − 373.62 = **121.38**.
- Post pieces total **320 mm**; the original 340 mm leaves **20 mm minus two kerfs**. The cut plan is feasible when both finished lengths are measured.
- Total extrusion: **3,124 mm**.
- New lengths: 2×226 + 2×186 + 2×100 = **1,024 mm**.
- Recutting two spare 110s into shelf rails reduces new stock to **824 mm**.
- DXF: closed outline, nine Ø5.5 holes and the R15 corner are present. Independent net-area calculation gives approximately **4,958 mm² and 40.2 g per plate**.
- Model counts: **24 long + 8 short brackets**, and **128 M5 screws**.

Evidence: [cutlist.md](/Users/joseph/scratch/frame-gate2/hardware/designs/torso-frame/review/packet2/data/cutlist.md), [gusset_plate_left.dxf](/Users/joseph/scratch/frame-gate2/hardware/designs/torso-frame/review/packet2/data/gusset_plate_left.dxf), and `scripts/frame.py`.

**MAJOR — Joey would still find these missing or undefined:**

- Positive step anchors and their hardware.
- Pack supports, stops, retainer and case attachments.
- Carrier-to-frame mounts and actual PCB standoffs.
- Jetson shelf/supports.
- Metal spine-pad load paths.
- Cable clamps, abrasion protection, terminal covers and connector holders.
- Actual resistor mounting hardware and heatsink arrangement.

The JSON contains **100 T-nuts**, not 128: 64 for corner brackets and 36 for gussets. The generator increments the count for 20 hardpoints and eight pitch screws without generating their nuts. Thus the fastener drawing does not show every claimed nut.

The photographs verify a mixture of printed corner reinforcement, metal angles and stool fixtures. They do **not** establish that 16 reusable metal 2028 brackets are fitted, nor confirm their dimensions or the complete loose inventory.

**MINOR — purchasing/document conflicts:**

- Shopping list says **three fans**; current model requires **four**. The board plan also provides three fan headers.
- Older fastener/detail text still specifies M5×16 for hardpoints and 28 long/four short brackets; current generator uses M5×20 and 24/eight.
- Pitch screw size and engagement remain assumptions.
- Buying another extrusion four-pack is not justified by length totals alone: two purchased packs total 4,000 mm, but remaining usable stick lengths are unknown.
- `bom.json` quantities combine assemblies and subparts: “two packs” means one pack plus its sense board; “12 boards/carriers” is not a purchasing quantity.

**Smallest fix:** one measured inventory and slot-by-slot build map, followed by a regenerated assembly BOM that includes the missing mounts. Do not order against the mixed revisions.

**7. Step joint — BLOCKER; adding the low pack does not resolve it**

The fall arithmetic is internally consistent:

\[
E=40(9.81)(0.75)=294.3\ {\rm J}
\]

\[
F_{\rm peak}=1.5(0.6E)/0.04=6.62\ {\rm kN}
\]

That corresponds to approximately **16.9 g**.

If the original 40 kg excludes the new pack, and the same effective drop and stopping distance are retained, load rises approximately **5.5%**. An 860 N bolt-share becomes approximately **907 N**. An assumed 1 kN friction capacity then has only about **1.10 nominal capacity/load ratio**.

Those scaling assumptions are not verified. A low pack may reduce overall centre-of-mass height, but it also adds approximately 367 N of local inertia to the lower frame. Its effect on each step joint depends on contact location and restraint. “Low mass” is not a reason to subtract it from the joint load.

More consequentially, the FEA bonds the joints perfectly and represents extrusion as tube. It does not establish T-nut slip, slot-lip strength, preload retention, post-end anchoring or load sharing between nine gusset screws and the brackets. The quoted 860 N also comes from a five-bolt sharing assumption, not an analysis of this fastening arrangement.

Evidence: [torso_stepped_FEA_README.md:50–86](/Users/joseph/scratch/frame-gate2/hardware/designs/torso-frame/review/packet2/data/torso_stepped_FEA_README.md:50).

**The rear crush path is another blocker.** The shell documentation deliberately makes panel bosses sacrificial, yet the crash pads depend on hardpoint/standoff load paths that are not designed. Four equally loaded pads would average roughly **1.65 kN each**; fewer initial contacts increase that. Their lateral offsets also impose bending on the supports. A 30 mm-proud TPU pad is not evidence of 40 mm usable stopping travel.

**Smallest fix:** finish the positive post anchors and a metal pad-to-frame load path, then assess the actual joint/contact model and absorber force–travel behaviour.

**Do not order final gussets before that.** Their hole locations, outline and neighbouring bracket positions may change. The R15 correction is present in the cutting file; it does not validate the revised joint.

**8. What could make cutting and ordering now a waste?**

The following can still change the cut geometry or custom parts:

1. **BLOCKER — Step anchoring:** a positive anchor may require different post ends, brackets, gusset holes or rail access.
2. **BLOCKER — Complete pack restraint:** the present envelopes reserve no demonstrated space for structural stops, latch, case mounting and protected connections.
3. **BLOCKER — Real electrical assembly:** the standoff stack is inconsistent, cable passages are occupied, and the lug/connector assemblies are not modelled.
4. **BLOCKER — Rear crash structure:** the pad supports and usable crush travel may require different hardpoints, clearances or reinforcement.
5. **MAJOR — Thermal packaging:** the pack heating estimate can require different airflow, operating limits or pack placement.
6. **MAJOR — Waist interface:** the 57 mm motor plus 6 mm adapter needs 63 mm, against a 40 mm bay. Calling it “hip stage” does not establish that its eventual solution preserves this torso interface.
7. **MINOR — Revision control:** the release README simultaneously contains torso-pack and pelvis-pack conclusions, three/four-fan layouts and different bracket/fastener lists.

The minimum useful release is a single reconciled assembly containing the actual anchors, pack restraint, board stack, routed harnesses, service envelopes and crash-pad supports. Until those fit together, the current post lengths and gusset DXF remain provisional.