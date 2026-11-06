**NO-GO on cutting the posts or ordering the gussets.** The supplied geometry contains assembly collisions and blocked cable routes. Correcting its fastener placement introduces board interference and obstructs battery withdrawal. Pack restraint, thermal management and the impact load path remain undefined.

I checked the 526 objects in [parts.json](/Users/joseph/scratch/frame-gate2/hardware/designs/torso-frame/review/packet2/data/parts.json), screening all 138,075 pairs, then resolving relevant candidates using individual bracket legs and cylindrical geometry. I also inspected the scripts, DXF, documents, rendered sheets and five photographs. Calculations were performed in memory; no deliverable files were created or edited, and no hardware was touched.

**Verified** below means verified against the supplied geometry or arithmetic. Actual extrusion profiles, manufactured clearances, joint strength, cooling performance and crash survival are **not verified**. The model lacks slots, many mounting features, connector bodies and several actual component shapes.

**1. Pack retention — BLOCKER**

At the packet’s assumed acceleration:

\[
F=2.2(17)(9.81)=367\ \mathrm N
\]

That is the pack’s inertial load before any safety factor, local impact amplification or collision after taking up clearance. The 17 g itself depends on achieving the assumed stopping distance.

| Direction | What the supplied model provides | Failure |
|---|---|---|
| Forward, +Z | Pack ends at Z34; insulating sheet occupies Z34–34.5; carrier begins at Z37.5 | No structural forward stop. Pack movement can load the carrier and electronics. |
| Backward, −Z | Sliding rails and D2, with magnets and an M3 run lock | No independent pack latch. Neither the door nor its hinges/inserts have a demonstrated 367 N load capacity. |
| Sideways, ±X | `packrail_±1` touches the pack at X±47.5 | Guides have zero nominal running clearance and no defined attachment/load path. |
| Upward, +Y | Pack top Y345; overhead wiring begins at Y345 | No hold-down. The pack can crush its own power/sense harness. |
| Downward, −Y | Rails extend Y180–348 beside the pack | The two rectangular rail envelopes contain no supporting ledge underneath the pack. Support is asserted, not designed. |

The mounting problem is worse than “printed rails need checking.” Each rail occupies \(|X|=47.5–55,\ Z=-50–40\). The front post starts at Z50, leaving **10 mm between rail and post**. At the rear post, the rail meets only the corner line \(X=±55,\ Z=-50\), not a mounting face. There are no ears, fasteners or other supports in the model.

Evidence: `parts.json`: `pack`, `packrail_±1`, `pdb_sheet`, `pdb_carrier`, `harness_pack`; [frame.py:299](/Users/joseph/scratch/frame-gate2/hardware/designs/torso-frame/review/packet2/scripts/frame.py:299); `shell_manufacturing_README.md` §§4–5.

**Smallest defensible fix:** a frame-mounted captive cradle with a bottom support, forward stop, side guides and hold-down, plus a removable positive rear retaining bar/latch. Let the printed pieces locate and cushion the pack; give retention a defined structural path to the extrusion. D2 should retain itself, not the battery. Dimension and verify the restraint for the selected crash pulse and margin before calling it sufficient. Adding a stronger magnet or another door screw does not establish that path.

**2. Electrical-mechanical safety — BLOCKER**

The **3 mm air gap is not crash protection**. It is the distance from the sheet’s front at Z34.5 to the carrier’s rear at Z37.5. No restraint limits pack motion or carrier deflection to less than that distance.

Credible faults include:

- Pack case or lid driving into the carrier, mounting hardware or exposed PCB conductors.
- Ring lugs rotating, or cable crimps rubbing aluminium after a restraint failure.
- Power and cell-tap wires pinched between the moving pack at Y345 and the shelf above.
- Front-corner trunks contacting brackets and screws: the present routes already intersect them.
- A resistor terminal or failed insulation contacting the bottom rail.

A 0.5 mm insulating sheet needs retained edges and protection against screws, abrasion and displacement. It does not make those mechanical faults acceptable. Moreover, its modeled edges intersect the tray rails, as listed under question 3.

The proposed **1 MΩ ∥ 10 nF connection is an EMC/bleed connection, not a protective fault-current bond**:

- At 54.6 V, the resistor passes only **54.6 µA**.
- The capacitor stores only **14.9 µJ**.
- A positive-to-frame fault would not clear a 60 A fuse through that connection.
- If another connection ties system ground to the frame, the same fault can instead become a high-current short.

Touching actuator housings does **not** establish whether their housings connect electrically to supply negative. That needs an actual continuity/isolation map, including USB shields and other mounted electronics.

The pack-side cable remains live upstream of the board-mounted BMS and E-stop. Neither switching device protects a short upstream of itself. The pack fuse and cell-tap fuses therefore need defined locations, fault ratings and protected unfused lengths.

Evidence: `README.md` §6; `pdb_PLAN.md` §§3.1, 4; `pdb_design_README.md` §1a; `parts.json` at the coordinates above.

**Smallest fix:** establish pack restraint first; add retained insulating barriers, covered terminals, lug anti-rotation and cable strain relief; route wires through genuinely clear, protected passages. Define the chassis fault strategy from actual electrical connectivity. Do not simply replace the RC network with a hard bond without analysing the resulting current paths.

The nominal motor-voltage arithmetic does fit the packet’s stated 24–60 V operating range: **46.8 V nominal, 54.6 V fully charged**. That does not verify regen protection. Also, \(13×4.15=53.95\) V, and the margin to a 57.5 V clamp is **3.55 V**, not the 0.7 V claimed in `pdb_PLAN.md` §3.4.

**3. Interference — BLOCKER for the current packaging release**

All coordinates below are millimetres in the packet’s torso frame. “Overlap” means overlap of the supplied envelopes; actual relieved parts require actual geometry to dismiss a finding.

For symmetric entries, apply both X signs and the indicated front/rear signs.

| Severity | Part IDs | Verified conflict or clearance | Smallest fix |
|---|---|---|---|
| Major | `rail_shelf_1` / `br25, br27`; mirrored `rail_shelf_-1` / `br29, br31` | **2 mm overlap** at \(|X|=73–75,\ Y353.62–373.62,\ |Z|46–50\). The gap between shelf and step-side rails is 18 mm; a 20 mm bracket still does not fit. | Relocate/reorient the brackets or specify a connection fitting the actual gap. |
| Major | `shelf` / `br24,26,28,30` | Shelf occupies the corner-bracket legs: \(|X|27–55,\ |Z|46–50\), and \(|X|51–55,\ |Z|22–50\), throughout Y354.6–372.6. Associated fasteners also occupy this envelope. | Model the actual shelf reliefs or relocate its brackets; include tool clearance. |
| Major | `pitch_wing_1` / `br3,9`; mirrored wing / `br15,21` | **1.62 mm vertical overlap**, Y400–401.62, \(|X|89–93,\ |Z|50–69\). The earlier change to the *top* brackets does not fix these *bottom* brackets. | Resolve against the real pitch-bracket shape; relocate or shorten the interfering connection. |
| Blocker | `harness_step` / `rail_shared_1` | The entire proposed passage at X33–45, Y353.62–373.62, Z50–60 lies **inside the shared front extrusion**. | Provide a real passage around the rail or a designed shelf opening. |
| Blocker | `harness_step` / `br2`, `f2_0_h/s`, `t2_0`; also `f24_0_s`, `t24_0` | Additional obstruction below/within that rail: bracket intersection X33–45, Y349.62–353.62, Z50–60. | Reroute the complete cable bundle, including connectors and bends. |
| Blocker | `harness_down_±1` / `rail_bot_1`, `rail_shared_1` | Ø8 trunks at \((X,Z)=(±51,66)\) pass through the bottom rail over Y160–175 and shared rail over Y353.62–370. | Move the trunks outside those members and reserve their turn/connector volumes. |
| Blocker | `harness_down_1` / `br0,2`; negative side / `br12,14` | Trunks intersect both bottom and step brackets, including their upright legs at \(|X|51–55\). | Resolve the bracket and trunk layout together. |
| Major | `harness_shoulder_±1` / `jetson` | Each shoulder run enters the Jetson envelope by **9.5 mm in X**: \(|X|42–51.5,\ Y447–455,\ Z-32–-22\). | Route outside the Jetson envelope, with the actual connector exit and bend radius. |
| Major | `pdb_carrier` / `packrail_±1` | Overlap at \(|X|47.5–50,\ Y190–340,\ Z37.5–40\). | Relieve/reposition the guides or carrier while preserving structural support. |
| Major | `pdb_sheet` / `packrail_±1` | Overlap at \(|X|47.5–50,\ Y190–340,\ Z34–34.5\). | Design the insulation/guide interface explicitly; do not leave an abrasion edge. |
| Major | `spine_rail` / rear `gusset_±1_-1` | Lip overlaps the plate near \(|X|55–60,\ Y355–372,\ Z-73–-70\). Rounded corners and holes do not remove the whole conflict. | Pocket the lip ends or shorten it clear of the gussets. |
| Major | `spine_upper_±1` / rear `ggusset_±1_-1_6_h`, `_7_h`, `_8_h` | Gusset heads reach Z−77.5; pads begin at Z−77: **0.5 mm overlap**, around \(X=±103,\ Y385/400/415\). | Provide actual pad/standoff recesses and distinguish rigid support from crush material. |
| Major | `pdb_m3_0…5_h` / corresponding `harness_down_±1` | All six modeled M3 heads intersect the front trunks near \(X=±46,\ Y194/265/336,\ Z61.5–63.2\). | Move the cable routes or mounting points and reserve driver access. |
| Major | `brake_res` / `packrail_-1` | Zero clearance at Y180 over X−55…−47.5, Z±25. A hot resistor directly meets a printed guide envelope. | Move the resistor or provide a thermally isolated, spaced mount. |
| Known, unresolved | `rs03_waist` / `pelvis_ghost`, `compute3` | Waist motor occupies Y92–149; it penetrates the pelvis placeholder to Y115 and overlaps `compute3` to Y120. | Finalise the hip interface and remove the fictitious third-board placement. |

Additional non-mating clearances below 2 mm:

| Part IDs | Clearance and location | Rank / smallest fix |
|---|---|---|
| `rs03_pitch_±1` / corresponding upper posts | **0.50 mm** at Z±50, near Y424.86 | Major: check actual motor diameter, assembly tolerance and loaded deflection. |
| Same motors / `rail_topside_±1` | **0.64 mm**, motor top Y474.36 versus rail underside Y475 | Major: establish a tolerance/deflection allowance. |
| Same motors / `rail_stepside_±1` | **1.74 mm**, motor bottom Y375.36 versus rail top Y373.62 | Major: same check. |
| Same motors / `br5,11,17,23` | **1.55 mm** minimum to the upper side brackets | Major: use actual bracket and motor geometry. |
| `pitch_wing_±1` / corresponding front/rear gussets | **1.00 mm** in Z, wing ends at ±69 and plates begin at ±70 | Major: verify relief and loaded clearance. |
| `harness_shoulder_±1` / `pitch_bracket_±1`, `pitch_wing_±1`, `rs03_boss_±1` | **0.40 mm** at \(|X|78–78.4\) | Major: replace the disconnected cylinder with a modeled connector and protected cable exit. |
| `pdb` / `br2,14`; `pdb_logic` / `br0,12` | **1.00 mm** laterally, board edge \(|X|50\), bracket \(|X|51\) | Major: move board/connection or supply real relieved geometry. |
| Those boards / `f2_1_h,f14_1_h,f0_1_h,f12_1_h` | **0.50 mm as supplied**; becomes interference after seating correction below | Major: correct fasteners before setting board width. |
| `pdb`, `pdb_logic` / `harness_down_±1` | **0.610 mm** actual cylinder-to-box separation | Major: reserve cable movement and insulation protection. |
| `harness_pack` / `shelf` | **1.00 mm**, Y353.6–354.6 | Major: include restraint, cable clips and actual wire/bend tolerances. |
| `harness_pack` / `br26,30` | Approximately **1.00 mm**, near Z−45/−46 and Y353.6/353.62 | Major: reroute or relieve after modeling the actual bundle. |
| Rear gusset heads `_4_h,_5_h` / upper pads | **0.13 mm**, heads end at Y367.87 and pads begin at Y368 | Major: provide pad relief. |
| `harness_down_±1` / front H3 brackets | **0.123 mm**, near Z70 | Major: keep cables clear of shell mounts. |
| `pi_stub` / `shelf` | **1.02 mm**, Y372.6–373.62; stub also touches the top of `br24` at X51–52, Z22–40 | Major: provide an actual insulated mount and bracket clearance. |

There are also zero-clearance interfaces requiring explicit treatment:

- `harness_side_±1` touches the front-post/bottom-rail corners, bottom brackets, PCB/carrier edges and tray rails throughout its proposed \(|X|50–55,\ Z40–50\) band.
- `harness_down_±1` is tangent to the front posts and front H4 mounts.
- The battery has **zero sliding clearance** between the two guide envelopes.
- Rear guide edges meet `br6,7,8,18,19,20`; the corresponding front bottom side brackets `br1,13` are only **1 mm** away.

Some of these contacts could be intentional support surfaces, but the packet does not provide the insulation, reliefs or sliding fits needed to make them acceptable.

**The fastener model prevents a trustworthy “all clear” result.** In [frame.py:169](/Users/joseph/scratch/frame-gate2/hardware/designs/torso-frame/review/packet2/scripts/frame.py:169), every corner-bracket screw is seated on the member-side face instead of the exposed face. Example: `br0` has a horizontal leg at Y175–179, but `f0_0_h` sits at Y175–179.5, largely inside the leg. Its head should start at Y179. The associated nut is displaced by 4 mm in the opposite sense.

Consequences, checked in memory using the packet’s screw dimensions:

- Correctly seated front upright heads extend inward to \(|X|46.5\), intersecting the 100 mm board envelopes.
- Rear upright heads intrude into the battery’s withdrawal path.
- Corrected bottom rear horizontal heads reach Y183.5, above the battery bottom Y183.
- Only **100 T-nut solids exist**, although the metadata claims 128. The 20 hardpoint and eight pitch-bracket nuts are absent.

The supplied model also contains cross-fastener intersections/near misses. For each corner group \(b=0,6,12,18\), with shelf bracket \(j=24+b/3\):

| As-supplied fastener family | Result |
|---|---|
| `f{b}_1_s` / `f{b+1}_1_s`; `f{b+4}_1_s` / `f{b+5}_1_s` | Intersecting perpendicular screw envelopes |
| `f{b+2}_0_s` / `f{j}_0_s`; `f{b+3}_0_s` / `f{j+1}_0_s` | Intersections in shared rails |
| Bottom bracket shanks / corresponding H4 and H6 shanks | Intersections |
| Lower-top bracket shank / corresponding H3 shank | Approximately 1.55 mm separation |
| Lower-post and upper-post bracket shanks / gusset shanks at Y337 and Y385 | Approximately 0.50 mm separation |
| Upper-bottom bracket shank / shared-rail gusset shank at X±84 | Approximately 1.41 mm separation |
| Those three bracket shanks / corresponding gusset nuts | Contact; associated bracket-nut/gusset-nut boxes are 1.5 mm apart |
| Two heads on each short bracket | Approximately 1.77 mm separation |
| Front upright bracket heads / adjacent board M3 heads | Approximately 1.86 mm separation |

**Rank: major model defect. Smallest fix:** correct the seating faces, add all nuts, use the purchased screw/nut geometry and stagger conflicting attachments. These original intersections should not individually be presented as proven manufactured collisions because their placement is wrong.

Importantly, correcting placement and adding the omitted nuts using the packet’s own 10 × 10 × 6 mm nut envelope produces **44 nut-envelope overlaps**. Examples include:

- Bottom bracket/H4 nuts: X36–45, Y167.5–170, Z62.5–65.
- Lower-step/shelf bracket nuts: X36–46, Y358.62–361.12, Z55–57.5.
- Lower-post/gusset nuts: X60–62.5, Y334.62–342, Z62.5–65.
- Upper-post gusset/pitch-bracket nuts: X98–100.5, Y419.7–420, Z62.5–65.

Their mirrored counterparts repeat around the frame. Actual nut shapes may reduce some overlaps; **a build order alone cannot resolve intersecting installed hardware**.

I excluded intended screw-to-hole/member engagement, motor subparts, mating frame faces and obvious placeholder containment from physical-failure counts. The solid pitch-bracket envelopes also overlap their motors and top-side rails; actual cutouts must replace those envelopes before a complete physical clearance claim is possible.

**4. Serviceability — MAJOR; “unplug then slide without removing anything else” is not established**

The nominal D2 opening passes the bare pack:

- Width: 120 − 95 = **25 mm**, or 12.5 mm each side.
- Height: 177 − 162 = **15 mm**: 8 mm below, 7 mm above.

But the rear frame hardware, not D2’s nominal rectangle, becomes the restriction.

With the correctly seated **4.5 mm-high heads specified by this generator**, `f6_1_h`, `f8_1_h`, `f18_1_h`, `f20_1_h` narrow the withdrawal passage to **93 mm**, less than the 95 mm pack. `f6_0_h` and `f18_0_h` also enter the swept pack volume by 0.5 mm from below. Actual shorter heads might improve this; they have not been specified or verified.

The claimed approximately 90 mm withdrawal is also insufficient to clear the frame completely: the pack’s front face starts at Z34 and must pass Z−70, requiring **at least 104 mm travel**. Passing a door plane at Z−82 would require 116 mm.

A workable service sequence must be demonstrated as follows:

1. Support the robot and pack; shut down and isolate the electrical system.
2. Open D2 and its run lock fully clear of the withdrawal path.
3. Access the connector latches while the pack remains positively retained.
4. Disconnect pack power, then the sense connector, following the specified electrical sequencing.
5. Release the independent pack retainer.
6. Withdraw the pack, sense board, connector holder and loose leads together without snagging the rear brackets.

**The present model fails step 6 under its stated fastener geometry.** Steps 3–4 are unverified because the XT90S, mated sense connector, holder, finger access and disengagement travel are absent.

For the boards, removal requires the front U3 panels and their fans to move out of the way. Then isolate power, disconnect pigtails and board interconnects, release the PCB/carrier fasteners and extract forward. However:

- Front brackets/heads obstruct the PCB envelopes.
- The carrier intersects the guides.
- Carrier-to-frame mounting and its release access are not designed.
- The modeled M3 screws run only from Z61.5 to Z53.5, never reaching the carrier at Z37.5–41.5.
- A 6 mm standoff placed on the carrier’s **front** at Z41.5 would put the PCB at Z47.5, not the modeled Z43.5.

Evidence: `parts.json`; `pdb_design_README.md` §1a; `shell_manufacturing_README.md` §5.

**Smallest fix:** define the actual mounting stack and extraction directions, then check swept volumes with real fasteners and mated connectors. Preserve rear pack service by relocating/recessing its obstructions rather than making structural-bracket removal a battery-change step.

**5. Cooling — BLOCKER for the claimed sustained-current packaging**

Air can plausibly turn around the boards and travel through peripheral shell spaces, but the proposed **central chimney is blocked**:

- Shelf: X±55, Z±50, Y354.6–372.6.
- Shared front rail: Z50–70.
- The alleged Z50–60 bypass is inside that rail.
- The Jetson occupies X±51.5, Y410–455, Z−45.5–45.
- Its support shelf and cables are not modeled.

Consequently, fan presence does not establish lower-to-upper throughflow. Upper exhausts could preferentially draw nearby leakage air while the lower battery cavity receives little cooling.

The largest missed heat source is the pack. Using the packet’s own 90 mΩ estimate:

| Pack current | Estimated resistive heat |
|---:|---:|
| 10 A | 9 W |
| 20 A | 36 W |
| 30 A | 81 W |
| 40 A | **144 W** |
| 60 A | **324 W** |

These are estimates, not measured thermal results. The underlying scale is independently supported by Molicel’s P50B typical DC impedance: \(13×12.8/2=83.2\) mΩ for the cells alone, giving approximately **133 W at 40 A**, before interconnect losses. The cell’s current rating does not guarantee adequate cooling inside this enclosure. [Molicel P50B datasheet](https://www.molicel.com/wp-content/uploads/Product-Data-Sheet-of-INR-21700-P50B-80122.pdf)

Thus `shell_manufacturing_README.md` §6’s “pack: a few W at 40 A” is wrong by roughly two orders of magnitude. Its 60–75 W whole-chest budget cannot support the specified 40 A thermal-soak case.

**Worst sustained hot spot:** likely the interior cells of the enclosed pack, whose side spaces are filled by guides and whose front is covered by insulation/carrier/boards. That is an engineering inference; temperatures require measurement. On the electronics, the upper BMS bank at Y294–318 extends above the intake ending at Y304.

The brake resistor is another serious issue:

- Modeled `brake_res` is only **25 × 50 × 8 mm**.
- The named HS50 family has a substantially larger envelope: approximately **28 × 72.5 × 14.8 mm**, including the manufacturer’s listed maximum dimensions.
- It requires a suitable heatsink interface; “bolted to a rail” is not a thermal calculation. [Ohmite/ARCOL HS data](https://www.ohmite.com/hs-aluminium-housed-resistors/)
- \(57.5^2/10=330.6\) W is **6.61 times** its nominal 50 W rating, not five times.
- A 40 W average allowance permits only approximately **12.1% clamp duty**, subject to verified pulse-energy and temperature limits.
- The current envelope directly touches the printed guide at Y180 and is only 3 mm below the pack.

The power-board FET arithmetic also understates heat: the listed hot resistances total 2.8 mΩ, yielding **2.52 W at 30 A**, not 1.6 W, before other losses.

**Smallest fix:** select the actual resistor and mounting arrangement; calculate the heat budget using RMS current and duty cycle; provide a defined battery heat-removal path and ducted airflow around the shelf. Verify closed-shell temperatures and flow before assigning sustained-current capability. A lower provisional current limit may avoid a frame enlargement, but the packet does not establish what that limit can be.

**6. Cut plan and shopping list — MAJOR inconsistencies; arithmetic alone does not release the build**

Verified extrusion arithmetic:

- \(198.62+20+121.38=340\) mm assembled height.
- Each old 340 mm post supplies 320 mm of finished post lengths, leaving **20 mm for two kerfs, trimming and offcut**.
- New stock: \(2×226+2×186+2×100=\mathbf{1024}\) mm.
- Final extrusion: **3124 mm**.
- Recutting the two spare 110 mm rails to 100 mm reduces new stock to **824 mm**.
- The generated built-frame model contains **2400 mm**, including two shelf rails. Without those rails, it is 2200 mm. The earlier FEA’s 2460 mm is inconsistent.

Two 500 mm sticks can each supply 226 + 186 mm with kerf allowance. The two 100 mm pieces need another stick or the spare 110 mm rails. Buying another four-pack “either way” is not justified without an inventory.

The photos show single extrusion members, metal angle brackets, corner pieces and stool attachments. They do **not** establish post lengths, every bracket’s material/model or the complete stock inventory.

The nominal revised hardware count is:

- 24 long plus 8 short corner brackets.
- 100 M5×10, eight M5×12 and twenty M5×20 screws.
- 128 M5 nuts.
- Sixteen collar M4 screws.

However, `FRAME_EXPLODED_DETAIL.png` still calls for **28 long/4 short brackets and M5×16 hardpoint screws**. The shell document calls for **40 M5×20 hardpoint fasteners**, while the frame models 20. These are purchasing/assembly discrepancies, not harmless captions.

Likely missing from the bench list:

- Pack cradle, positive retainer and their structural fasteners.
- Carrier-to-frame attachments and six actual PCB standoffs.
- Jetson shelf/supports and hardware.
- Resistor mounting adapter, thermal interface and M3 T-nuts.
- Cable clamps, insulating guards, connector holders and strain relief.
- Metal spine-pad load-path hardware.
- Final step-joint anchors and their fasteners.
- **Four fans**, whereas the shopping list specifies three; electrical plans also still describe three fan headers.

The “50 drop-in nuts” proposal can cover 36 gusset + four H3 + eight pitch nuts, but only if the remaining attachments are deliberately preloaded. That does not resolve the installed nut overlaps identified above.

**Smallest fix:** reconcile one hardware schedule against actual bracket/nut/screw geometry and a slot map; then inventory existing stock. Include a dry assembly before cutting the four existing posts.

**Minor document/BOM faults:** `bom.json` reports two batteries because it counts the pack and sense board separately, twelve “power and logic boards” because it counts their subparts, and five H3 brackets because it includes the lip. The parts table also retains obsolete battery and third-compute-board options. These counts are not purchasing quantities.

**7. Step joint — BLOCKER, and the low battery does not excuse it**

Recalculation of the FEA’s assumed impact basis gives:

\[
40(9.81)(0.75)=294.3\ \mathrm J
\]

\[
F_\text{peak}=1.5\frac{0.6(294.3)}{0.04}=6.62\ \mathrm{kN}
\]

That reproduces approximately 17 g. It does not validate the assumed 40 mm stopping stroke.

The extra pack contributes approximately **367 N** at that acceleration. Its centre is Y264, about 99.6 mm below the shared-rail centre Y363.62, producing roughly **36.5 N·m additional local moment** in a transverse impact. A lower centre of mass can affect the whole-body fall trajectory, but it does not eliminate this local load.

If the original 40 kg excludes the pack, adding 2.2 kg increases total mass by **5.5%**, under otherwise unchanged assumptions. Relative to the stated 5.3 kN lower-body load, 367 N is approximately another **6.9%**. If the 40 kg already includes the pack, those percentage increases must not be added again; the packet does not establish the accounting.

The assumed slip margin is already only:

\[
1000/860=\mathbf{1.16}
\]

That is before preload variation, unequal bolt sharing, prying, wear or impact amplification. The FEA divides a 4.3 kN plate load over five bolts, while the released plate has nine holes; there is no released bolt-group calculation explaining which bolts react which forces.

The back-fall case is worse than “at the limit”:

- Reported 99th-percentile stress is **146 MPa versus 145 MPa assumed yield**, even with bonded joints.
- The pad coupon criterion permits only **25 mm crush**, while the frame calculation assumes 40 mm total stop.
- At only 25 mm total stop, the same scaling predicts roughly **234 MPa** back-frame stress.
- A 30 mm-thick pad is not automatically 30 mm of usable controlled crush.
- Metal pad standoffs/load spreaders are requested by the shell document but absent from the frame model and shopping list.

Evidence: [torso_stepped_FEA_README.md](/Users/joseph/scratch/frame-gate2/hardware/designs/torso-frame/review/packet2/data/torso_stepped_FEA_README.md), “FEA so far” and “Tip-over against crush distance”; `shell_manufacturing_README.md` §§8–9.

**Do not order the gussets before designing the positive anchor.** The anchor may change their outline, hole locations, access or local load distribution. Adding more friction-only brackets does not establish a positive shear path.

**Smallest fix:** dimension the end/bearing anchor and pad-to-frame load path, calculate the actual bolt group with the pack included, and verify the selected extrusion/nuts and crush force–travel response. The existing bonded-body FEA cannot release those joints.

The DXF itself contains nine Ø5.5 holes and the requested R15 corner. I calculate approximately **40.16 g per plate**, consistent with the rounded 40 g BOM value. The generator’s area calculation mishandles the concave fillet, but its approximately 0.22 g error is **minor**; structural release is the issue.

**8. What would make cutting and ordering now a waste?**

The changes most likely to invalidate the current parts are:

1. **A real pack cradle and connector/fuse arrangement exceeding the assumed pack envelope.** The pack-build document declared authoritative is not included, and the fuse, XT90S holder and complete mated connectors are not modeled.
2. **A wider service passage** required after correct fasteners, retention, insulation and sliding tolerances are included.
3. **A changed shelf or rail arrangement** to resolve the four shelf-bracket collisions and create actual cable/air passages.
4. **A larger resistor or battery cooling path** that cannot fit the lower bay.
5. **A positive step anchor** requiring different gusset holes, plate geometry or post-end preparation.
6. **A revised impact load path/crush system** requiring different hardpoints or more frame clearance.
7. **A hip interface that cannot accept the present bottom rectangle without changing its height or attachments.** The current waist adapter is a placeholder, not an accepted interface.

The cut lengths are arithmetically consistent with the current rectangular layout. **That layout has not yet survived an assembly, service, thermal or joint-load check.** Release the cuts and gussets only after those interfaces are dimensioned and the collisions above are resolved.