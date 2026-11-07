The current set has a definite fastener assembly conflict, an unresolved battery escape path, and unsupported features in the suggested print orientation.

I verified dimensions from the construction scripts and independently decoded the STL bounds and volumes. The carrier is **38,279 mm³ / 48.61 g**, the hooks total **4,653 mm³ / 5.91 g**, and each side is approximately **55,434 mm³ / 70.40 g**, using the stated density. I did not reproduce the voxel checks or establish mesh watertightness. No files were changed and no hardware was touched.

The strength calculations below are screening models, not measured capacities. Where I use **10–30 MPa**, that is an explicitly assumed effective stress limit for sensitivity analysis—not a verified property of these prints.

**1. Load case and first failure — BLOCKER: sacrificial parts do not have independent battery containment.**

For the specified pack:

\[
F=2.2(9.81)n=21.58n\ \mathrm N
\]

Thus 10 g gives 216 N and 20 g gives 432 N. “Tip-over from standing” does not define a peak acceleration. As an illustrative energy calculation, an equivalent 0.8 m fall stopped over 40 mm produces approximately **20 g average deceleration**, before accounting for the peak pulse. Actual robot rotation, impact direction and crush travel are missing.

Using the geometry in [build_cradle.py](J:/code/marengo/hardware/designs/torso-frame/review/packet3/scripts/build_cradle.py:127):

| Path | Independently calculated screening result | Implication |
|---|---|---|
| Upward load through hooks | The bar through each web is 12 × 3 mm: section modulus \(S=bt^2/6=18\ \mathrm{mm^3}\). For illustrative equal sharing, each carries \(10.79n\) N. The longer hook has roughly 43 mm overhang; a distributed reaction with 21.5 mm lever gives \(\sigma=12.9n\) MPa. | Reaches the assumed 10–30 MPa limit at **0.8–2.3 g**. Loading nearer its tip doubles the stress. Actual contact distribution and screw load sharing could change this substantially. |
| Downward load through ledges | With each side carrying half, 79 mm bearing length, 4 mm thickness and approximately 4.25 mm lever: \(\sigma\approx0.218n\) MPa. | The isolated ledge reaches that screening limit at roughly **46–138 g**; its supporting foot is more concerning. |
| Bottom foot/root | Taking the 43 mm mounting tongue, 4 mm thickness and approximately 11.25 mm load eccentricity to the rail’s inner edge gives \(\sigma\approx1.06n\) MPa. | Roughly **9–28 g**, before local concentrations, imperfect load spreading or print defects. |
| Forward load through front lips | Distributed across approximately 162 mm height, 3.5 mm thickness and 3.25 mm lever gives \(\sigma\approx0.106n\) MPa. | The lip alone is unlikely to govern under this distributed-load assumption. Its web connection and concentrated pack contact remain unqualified. |
| Lateral load through a web | An optimistic unperforated, fixed-ended 178.62 mm span, 87.5 mm wide and 3.5 mm thick, with a central resultant gives about \(2.70n\) MPa. | Roughly **4–11 g** in this simplified model. Windows weaken it; edge stiffening and distributed contact change the result. |
| Rearward load through thumbscrews and pack inserts | With equal sharing, each attachment carries \(10.79n\) N: **216 N at 20 g**, plus preload and any prying. | No defensible failure g is possible without the pack-case wall, insert geometry, installation and pullout/shear data. |

These are separate hypothetical load paths, not a solved assembly model. In particular, an M4 shank in a Ø4.5 hole has only 0.25 mm nominal radial clearance, while the hooks have 0.5 mm pack clearance. The screws may therefore take uplift before the hooks engage.

**Likely first printed failure:** the long hook/root if the hooks carry appreciable uplift; web bending or the pack insert/boss connection in other directions. A unique first-failure acceleration cannot honestly be established.

**Capture after failure is not demonstrated.** The rear is intentionally open, the hooks permit rearward pack sliding, and the frame README explicitly says the D2 door is not a restraint. Losing the side attachment path can release the pack toward −Z. Printed feet breaking can also release the pack together with its cradle.

**Smallest fix:** add an independently anchored restraint or rear keeper that captures the pack after printed-part failure, avoids loading the BMS/connectors, and does not depend on the pack’s heat-set inserts. Define its load and permitted travel.

**2. The 4.7 mm strip — MAJOR: inadequate evidence for a locally pried attachment.**

The actual roof thickness is:

\[
353.62-348.9=4.72\ \mathrm{mm}
\]

It is **not a free 55 mm cantilever**: much of it joins the continuous top foot. Nevertheless, that does not establish adequate strength.

If the longer hook’s illustrative moment remains concentrated over its 12 mm bar width:

\[
S_{\rm roof}=12(4.72)^2/6=44.6\ \mathrm{mm^3}
\]

\[
M=10.79n(21.5)=232n\ \mathrm{N\,mm}
\]

That gives approximately **5.2n MPa**, or **104 MPa at 20 g**, before allowing for load spreading. The adjacent foot is only 4 mm thick, and the slot terminates without a specified corner radius. Hook rocking adds localized contact and prying.

**Smallest fix:** close the unnecessary rear-open portion of the slot. Side insertion only needs a local aperture around the hook’s Z −15 to +5 envelope. Restore the rear web connection, radius the aperture ends, and provide a short reinforced load path into the top foot. This improves the geometry; it does not itself establish impact capacity.

**3. Thumbscrew bowing — MAJOR: tightening is being used to consume assembly clearance.**

The 0.5 mm deflection need not immediately break a web. An ideal, unperforated beam calculation with \(E=2{,}000\ \mathrm{MPa}\) gives:

\[
I=87.5(3.5)^3/12=312.6\ \mathrm{mm^4}
\]

For a 178.62 mm span, moving its centre 0.5 mm requires approximately **2.6 N with simple supports or 10.5 N with fixed ends**. The real windows, lip and pads invalidate either as an exact stiffness prediction.

The problem is what happens after the gaps close. With the illustrative screw relation \(T=KFd\), \(K=0.2\), even **0.1 N·m on M4 corresponds to 125 N preload**. Tightening can rapidly change from gently bending the web to pulling on the pack insert and compressing the boss/case. Creep then changes the clamp force.

The stated screw reach is initially correct: \(16-(59-47.5)=4.5\) mm. Bowing the boss inward by 0.5 mm increases this to **5 mm**, leaving only 1 mm before the specified penetration limit. Plastic crushing is not a reliable length stop.

**Smallest fix:** provide fitted clearance spacers and a positive compression/penetration stop with a specified thumbscrew. Use a controlled tightening instruction afterward; torque control alone does not remove the tolerance or creep problem.

Also, do not apply the frame BUILD sheet’s approximately **4 N·m** metal-joint guidance to these printed feet. With the same torque approximation, that is 4,000 N per M5 screw. Even an optimistic 33 mm² head-bearing area implies about 120 MPa, before subtracting the slot opening. Specify load spreaders or compression limiters and recheck screw engagement.

**4. Snap hooks — MAJOR: nominal snap arithmetic does not establish retention.**

The [hook description](J:/code/marengo/hardware/designs/torso-frame/review/packet3/data/cradle/README.md:63) gives \(L=17.8\), \(t=1.6\), \(b=7.2\), and required deflection \(\delta=1.1\) mm. For a simple end-loaded cantilever:

\[
\epsilon_{\max}=\frac{3t\delta}{2L^2}=0.00833=\mathbf{0.83\%}
\]

\[
P=\frac{Ebt^3\delta}{4L^3}=\mathbf{2.88\ N}
\]

Those reproduce the claimed approximately 0.9% and 3 N. They omit the thickened barb, root concentration, friction and print variability.

The nominal fit arithmetic is also internally plausible:

- Slot clearance: \(3.4-3.0=\mathbf{0.4\ mm}\).
- Total lateral float between barb and lip: **0.4 mm**.
- Pin engagement: \(51.5-50.9=\mathbf{0.6\ mm}\), reducing to **0.4 mm** at outward float.
- Inward float consumes the entire **0.2 mm pin-to-pocket-floor clearance**.
- Depressing the tongue 1.1 mm leaves nominally **0.8 mm above the pack**.

That last clearance only describes the tongue’s final displacement. It does not prove blade access, operation beside the installed harness, or release while the pack loads the hook.

The **3 N is an actuation force**, not a retention rating. The 12 × 3 mm bar, lip, barb and web must resist the much larger forces associated with cantilever moment. A 232 N·mm moment reacted across a roughly 7 mm contact couple already requires approximately **33 N contact force at the illustrative 1 g load**.

**Smallest fix:** remove the hooks from the primary crash-retention path. For their remaining locating function, test a full-size hook with a representative slot coupon, including insertion, release, rocking and hot/cycled retention. Add root radii and tolerance allowance based on that result.

**5. Board carrier — BLOCKER: the opposing screws cannot seat as specified.**

The [carrier fastener stack](J:/code/marengo/hardware/designs/torso-frame/review/packet3/data/carrier/README.md:11) leaves only 2 mm of plastic under each counterbore:

\[
4-2=2\ \mathrm{mm}
\]

A rear M3×6 therefore enters the 6 mm standoff by **4 mm**. A front M3×6 through an assumed 1.6 mm PCB enters by **4.4 mm**:

\[
4+4.4-6=\mathbf{2.4\ mm\ interference}
\]

The front shank position in `parts.json` is consistent with that 1.6 mm board assumption. A through-tapped standoff has colliding screw tips; a blind-ended version may bottom even earlier.

**Smallest fix:** specify the actual standoff and shorter screws. M3×4 on both sides would give approximately 2.0 and 2.4 mm engagement, leaving 1.6 mm between tips, subject to verification of thread depth and sufficient engagement. Increasing standoff height would disturb the board keep-outs.

**MAJOR — thin material around fastener seats.** From [build_carrier.py](J:/code/marengo/hardware/designs/torso-frame/review/packet3/scripts/build_carrier.py:13):

- Countersink mouth-to-side-edge ligament: \(55-51.75-3.1=\mathbf{0.15\ mm}\).
- End counterbore-to-plate-edge ligament: \(194-190-3.25=\mathbf{0.75\ mm}\).
- Counterbore floor thickness: **2 mm**.

The countersink’s 0.15 mm is the surface ligament, not a through-thickness ligament, but it is still vulnerable to installation damage and conical wedging.

At 20 g, the board produces approximately **49 N**, or 8.2 N per standoff under equal sharing. Including the carrier gives approximately **59 N**, or 14.7 N per cradle attachment. Those loads do not by themselves condemn the whole plate, but they do not establish vibration life, insert pullout capacity or resistance to unequal component loads.

**Smallest fix:** add local material around the mounting mouths and end counterbores, specify insert dimensions and tightening limits, and restrain cable loads independently of the PCB.

**Interference findings:**

- Carrier-to-side-cable-band nominal Z clearance is **0.5 mm**. Flush countersinks are essential.
- The specified bracket heads begin at Z 55; the carrier ends at Z 41.5 and standoffs at Z 47.5. There is **no nominal carrier/standoff collision with those heads**. Tall corner components remain subject to the board keep-out.
- Mount-to-standoff centre distance is **8.31 mm**; the two modeled recess radii leave approximately **1.96 mm** between them.
- The rear button heads overlap the cradle pads in plan. Their nominal recess margin is only \(2-1.65=\mathbf{0.35\ mm}\). Counterbore sag or an incompletely seated screw can prevent the carrier sitting flat.
- A 4 mm plate is not verified as sufficiently rigid under shock. A simple 21 × 4 mm side-strip model spanning 130 mm gives approximately **1.7 mm deflection** under an 8.2 N middle-standoff load. The real crossbars and attachments change that estimate, but the 3 mm rear air gap should not be treated as guaranteed under load.

**6. Thermal — MAJOR: the clearances are not a cooling provision.**

Neither 0.5 mm beside the pack nor 3 mm behind the carrier establishes airflow. The frame documentation still leaves lower-box exhaust unresolved, and other torso electronics add heat.

The pack’s total rectangular surface area is:

\[
A=2(0.095·0.162+0.095·0.079+0.162·0.079)=0.0714\ \mathrm{m^2}
\]

For an illustrative effective surface heat-transfer coefficient of 5–10 W/m²K:

\[
\Delta T=\frac{36}{hA}=\mathbf{50–101\ K}
\]

This is not a predicted operating temperature: conduction, radiation, blocked faces and shell-to-room transfer need a real thermal model or test. It demonstrates why “36 W” cannot be dismissed.

For context only, Bambu’s PETG HF datasheet lists heat-deflection temperatures of **62°C at 1.8 MPa and 69°C at 0.45 MPa**. These are short-term specimen results, not continuous-use limits for these preloaded parts, and the packet does not identify PETG HF as the selected material. [Manufacturer datasheet](https://www.additive-x.com/shop/pub/media/mageplaza/product_attachments/attachment_file/b/a/bambu_petg_hf_technical_data_sheet_4.pdf)

**Smallest fix:** identify the filament, define an allowable loaded-part temperature, and establish a cooling path. For illustration, holding the pack surface below 50°C at 25°C ambient with 36 W requires total thermal resistance below **0.69 K/W**. Verify temperatures and retention creep together before powered use.

**7. Printing — MAJOR: the side orientation needs supports that are not specified.**

Standing a side on Z −50 puts these features above the bed:

| Feature | First appears above bed | Unsupported projection |
|---|---:|---:|
| Ledge | 5 mm | 8 mm inward |
| Top foot | 6 mm | 23.5 mm outward |
| Bottom mounting tongue | 28.5 mm | 20 mm outward |

These are one-sided overhangs, not bridges between two supports. A rear-edge orientation preserves some useful layer directions but does not make the side self-supporting.

**Smallest fix:** establish and inspect a supported slice before printing the full sides. Keep support damage away from bearing faces and inspect the ledge/foot roots. “Four walls, 15% gyroid” is not a complete structural print specification.

For the hooks, the **21.7 mm bridge crosses a nominal 0.8 mm separation from the tongue**. Sag can fuse the flexure or change its release force. The tongue and barb also begin at the bed-facing surface, making first-layer enlargement relevant to the 0.2 mm clearances. Print the hook/slot coupon before committing to a full set.

The carrier’s flat orientation is feasible in principle, but its downward-facing counterbores must produce genuinely recessed screw heads. Inspect those seats; the available 0.35 mm head recess is small.

Finally, placing the C-section in the layer plane does not put **every** impact load in-plane. Fore/aft loads still challenge interlayer strength.

**8. Contradictions and incomplete evidence — MAJOR/MINOR as noted.**

| Severity | Evidence | Smallest fix |
|---|---|---|
| Major | Supplied check scripts look for `generator/parts.json` and STLs beside the scripts. The packet instead stores these under `data/frame`, `data/cradle` and `data/carrier`. `mass_estimate.py` would find no STLs in `scripts/`. | Make the checks accept explicit packet paths and fail when inputs are absent. Existing check claims are not reproducible from this layout unchanged. |
| Major | `draw_carrier.py` imports `build_carrier.py`, which exports files at import time. The apparent drawing utility is not read-only. | Move export operations behind an explicit entry point. I did not execute it. |
| Major | The cradle README still reports a real insulating-sheet overlap requiring narrowing to \|X\| ≤42, but supplied `parts.json` already uses ±42. | Reconcile the recorded check with the current geometry. Do not count the stale overlap as an outstanding collision. |
| Major | The frame table gives 2,212 g for pack plus BMS, while JSON lists 2,200 g pack +180 g BMS +30 g XT90 +10 g XT60. | Define the actual restrained assembly mass. If additive, it is at least **2.42 kg**, raising the calculated inertial loads by 10%. |
| Major | JSON omits detailed hook retention geometry and several new fasteners; only six board screw assemblies are explicitly modeled despite requiring twelve standoff screws. The assembled image omits the carrier, brackets and harness. | Include actual hardware and retention envelopes in the assembly check. The images cannot establish complete fit. |
| Minor | The frame’s 96 M5/T-nut count is frame-only; the cradle adds eight. The carrier requires **16 M3 screws**: four mounts, six rear and six front. | Publish one assembly BOM: **104 M5/T-nut pairs** with this cradle, plus the separate end-tap screws. |
| Minor | Frame prose retains two-PCB/on-board-BMS descriptions and the old X −35…30 lead route; current data uses one board, on-pack BMS and X −5…25. The fan shopping count is three while the model specifies four. | Remove superseded packaging and procurement instructions. |

The original cradle spec’s changed web position, feet and strap are explicitly documented revisions. The more consequential discrepancies are the stale check results, incomplete hardware model and unreconciled assembly mass.

**NO-GO for printing the current first test set.**