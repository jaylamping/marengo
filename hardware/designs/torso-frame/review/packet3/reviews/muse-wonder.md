**Result: do not print the first test set yet.** Two geometry defects will likely waste or break it, and the pack is not captured in the design tip-over case. No files were changed.

## Verified vs assumed

**Verified independently from the packet files:**

- STL bounds and volumes from the binary meshes: cradle side 55,434 mm³ / 70.4 g solid; strap 4,653 mm³ total, split 2,927 mm³ right / 1,726 mm³ left; carrier 38,279 mm³ / 48.6 g. SHA-256 values sampled match [SHA256SUMS.txt](/J:/code/marengo/hardware/designs/torso-frame/review/packet3/SHA256SUMS.txt:1).
- Web lightening: two stadium windows total ≈5,331 mm² on an 87.5 × 178.6 mm web = 34 %, matching the claim in [data/cradle/README.md](/J:/code/marengo/hardware/designs/torso-frame/review/packet3/data/cradle/README.md:16).
- Tongue arithmetic: `I = 7.2·1.6³/12 = 2.46 mm⁴`, `F = 3EIδ/L³ = 2.9 N`, strain `3tδ/2L² = 0.83 %` at δ=1.1 mm, L=17.8 mm — the author's 0.9 % / 3 N in [data/cradle/README.md](/J:/code/marengo/hardware/designs/torso-frame/review/packet3/data/cradle/README.md:84) is arithmetically right.
- Carrier edge ligaments from [scripts/build_carrier.py](/J:/code/marengo/hardware/designs/torso-frame/review/packet3/scripts/build_carrier.py:14): countersink rim to plate side edge = 55 − (51.75+3.1) = **0.15 mm**; standoff counterbore to top/bottom edge = 194−3.25−190 = **0.75 mm**, and 340−(336+3.25) = **0.75 mm**.
- Standoff screw thread engagement: M3×6 through a 4 mm plate leaves **2 mm** in the standoff.
- Pack force scale: 2.2 kg = 21.6 N at 1 g; 216 N at 10 g; 432 N at 20 g; 647 N at 30 g.

**Assumed, because the packet does not provide it:** printed PETG allowable 25–45 MPa (E = 2 GPa is the author's value in [data/cradle/README.md](/J:/code/marengo/hardware/designs/torso-frame/review/packet3/data/cradle/README.md:84)); tip-over peak deceleration from COM drop 0.3–1.0 m over 5–20 mm crush = roughly 30–120 g peak, so even a gentle 10–20 g is a fair screening case; pack-case insert strength, pack case thickness, shell inner clearance, and T-nut thread depth are **not in the packet** and are treated as unknown, not as passing.

## Ranked findings

### Blockers

**B1 — Strap hooks fail in uplift at ~1–3 g; the pack then is not captured upward.**
Each hook is an independent cantilever since the lead gap splits the strap ([data/cradle/README.md](/J:/code/marengo/hardware/designs/torso-frame/review/packet3/data/cradle/README.md:61)). Through the web the bar is only 12 (Z) × 3 (Y) mm ([data/cradle/README.md](/J:/code/marengo/hardware/designs/torso-frame/review/packet3/data/cradle/README.md:86)), so `S = 12·3²/6 = 18 mm³`, `M_allow = 630–810 N·mm` at 35–45 MPa. Right hook carries 42.5/65 = 65 % of uplift at a 21.75 mm lever from the web; left carries 35 % at 11.75 mm. Right-hook yield is reached at a total pack uplift of only **44–57 N = 2.0–2.6 g**, before any stress concentration at the tongue slit or slot corners. With Kt≈2 the margin is ~1 g — i.e. close to the static upside-down weight. *Smallest fix:* restore a continuous load path over the pack (reroute leads, or add a bolted steel/PETG cross-bar joining the two hooks), or enlarge the through-web section and add a second retention feature; do not rely on these cantilevers.

**B2 — Carrier screw and standoff holes break out of the plate edges.**
Countersunk carrier screws at (±51.75, 200/330) leave 0.15 mm of PETG to the X edge; top/bottom standoff counterbores at Y 194/336 leave 0.75 mm to the Y edges ([scripts/build_carrier.py](/J:/code/marengo/hardware/designs/torso-frame/review/packet3/scripts/build_carrier.py:15)). Tightening an M3 flathead on a 0.15 mm rim, or a button head on a 0.75 mm rim, will crack the rim on first assembly. *Smallest fix:* grow the plate (e.g. Y 186–344 and/or X to ±58 after re-checking cable/post clearances), or add local bosses around the six standoff holes and four carrier screws; keep ≥2.5 mm ligament.

**B3 — Rearward (−Z) capture rests only on two M4 thumbscrews into pack-case inserts of unknown strength.**
The pack slides out toward −Z by design ([PACKET.md](/J:/code/marengo/hardware/designs/torso-frame/review/packet3/PACKET.md:9)); there is no rear stop, and the D2 door is explicitly not a restraint in [data/frame/README.md](/J:/code/marengo/hardware/designs/torso-frame/review/packet3/data/frame/README.md). Rearward inertia shears the M4 screws across the boss hole (Ø4.5 in Ø16 boss, [scripts/build_cradle.py](/J:/code/marengo/hardware/designs/torso-frame/review/packet3/scripts/build_cradle.py:143)) and the heat-set inserts in the pack case, for which the packet gives no material, wall, or pull/shear data. *Smallest fix:* add a positive rear stop (removable rear bar or ledge extension) so the thumbscrews only locate sideways; otherwise supply insert/case test data.

**B4 — The stated side print orientation is not printable as written.**
“Stand each side on its rear edge” ([data/cradle/README.md](/J:/code/marengo/hardware/designs/torso-frame/review/packet3/data/cradle/README.md:55)) puts the part on the web's 3.5 × 178.6 mm rear face at Z=−50; every other feature starts at Z=−44 or later ([scripts/build_cradle.py](/J:/code/marengo/hardware/designs/torso-frame/review/packet3/scripts/build_cradle.py:127)). The centre of mass sits outboard of that 3.5 mm strip (feet reach |X| 75), so the part will peel or topple without a brim/fixture, and no brim, supports, temperatures, or wall/infill settings are specified for the sides. *Smallest fix:* specify brim + orientation fixture and print one side first as the test, or choose a flat orientation and re-derate layer-direction strength.

### Majors

**M1 — The 4.7 mm strip over the strap slot is the only link from the lower web to the top foot for Z −50 to 5.2, with sharp slot corners.**
Slot top Y 348.9 to web top 353.62 = 4.72 mm ([scripts/build_cradle.py](/J:/code/marengo/hardware/designs/torso-frame/review/packet3/scripts/build_cradle.py:169)); the top-root chamfer exists only forward of Z 5.2 ([scripts/build_cradle.py](/J:/code/marengo/hardware/designs/torso-frame/review/packet3/scripts/build_cradle.py:141)), and the rear top M5 at Z=−30 sits above the slot. Uniform tension over the hook's 12 mm through-web length gives 3.5×12 = 42 mm² (≈1,680 N ideal), but the load jogs in X from web (48–51.5) into the foot (51.5–75), so peel/bending and the un-radiused box-cut corners govern, not uniform tension. It will likely outlast the hook (B1) and then become the next failure. *Fix:* integral outer rib over the slot (|X| 51.5–54.5, Y 344–354, Z −50 to 10, re-checking the |X| 55 post line), radius slot ends ≥2 mm, or split the slot with a mid bridge.

**M2 — Sideways (±X) web bending at the windows is the next weakest path, order 4–10 g.**
One-way beam estimate for the 3.5 mm web spanning Y 175–353.6: `I = 313 mm⁴`; 100 N on one web gives 12.5 MPa full-section, ×3.43 at the window ligaments (9 + 16.5 mm remain) = 43 MPa, before Kt. Two-way plate action from the lip/ledge will raise this, but no FEA exists ([PACKET.md](/J:/code/marengo/hardware/designs/torso-frame/review/packet3/PACKET.md:32)). *Fix:* thicken the window ligaments, shrink windows near mid-height, or accept and instrument a drop test; do not quote a g rating without FEA or test.

**M3 — Thumbscrew preload is uncontrolled; the webs are the spring.**
A one-way estimate gives ~3 N for 0.5 mm centre bow (two-way plate: order 10 N), while hand torque on M4 easily reaches 0.4–1.2 kN clamp. Sequence: webs bow in, touch the pack after 0.5 mm, then further torque spikes into the pack-case inserts; PETG creep then relaxes the clamp and the pack rattles on its 0.5 mm float, working the screws loose. Tip penetration grows from 4.5 mm toward the 6 mm limit when a web bows 0.5 mm with the pack shifted. *Fix:* one side as fixed datum, the other spring-loaded or shimmed; add a shoulder/stop collar, a “snug + fraction of a turn” spec, and foam/PTFE side pads. Verify bow with feeler gauges at assembly.

**M4 — Hook fit/hold margins are inside normal FDM tolerance.**
Seated clearances are 0.2 mm at the web and 0.4 mm in slot height; barb catch bears only on Y 349–350; detent engagement is 0.6 mm, 0.4 mm at worst float ([data/cradle/README.md](/J:/code/marengo/hardware/designs/torso-frame/review/packet3/data/cradle/README.md:67)). A ±0.2–0.3 mm print/slot error, or sag on the 21.7 mm bridge, can consume all of it. The 3 N snap is easily overcome by harness tug or vibration. *Fix:* print a hook-plus-slot coupon first, open slot height by 0.2 mm, deepen the detent, and add a positive lock (e.g. a through pin) if hooks remain.

**M5 — Carrier: 2 mm standoff-screw engagement, thin insert pads, and a trapped assembly order.**
Standoff screws engage 2 mm (above); carrier pads are 7.5 × 10 mm with a 4.2 pilot, leaving ~1.3 mm walls around a heat-set insert in a 3.5 mm web's front edge ([data/cradle/README.md](/J:/code/marengo/hardware/designs/torso-frame/review/packet3/data/cradle/README.md:31)). Rear standoff heads sit partly under the pads ([data/carrier/README.md](/J:/code/marengo/hardware/designs/torso-frame/review/packet3/data/carrier/README.md:14)), so standoffs cannot be serviced without unbolting the carrier. *Fix:* longer rear screws (M3×8) with deeper counterbore, thicker pads, and confirm insert pull-out by test.

**M6 — Thermal creep risk at 36 W in a closed lower box is unclosed.**
Webs sit 0.5 mm from the pack and the carrier 3 mm in front of the insulating sheet ([PACKET.md](/J:/code/marengo/hardware/designs/torso-frame/review/packet3/PACKET.md:49)); the lower box has no proven exhaust in [data/frame/README.md](/J:/code/marengo/hardware/designs/torso-frame/review/packet3/data/frame/README.md). PETG creeps well below its ~80 °C Tg; at a plausible 50–65 °C local temperature the pre-bowed webs, the 4.7 mm strip, and the snap tongues will relax, loosening retention over hours. *Fix:* instrument the first powered run with thermocouples on web, strap, and carrier at 36 W worst case; provide the side-grille exhaust path before sustained running.

**M7 — The author's checks cannot be re-run from this packet, and the render omits the risky neighbours.**
[scripts/check_overlaps.py](/J:/code/marengo/hardware/designs/torso-frame/review/packet3/scripts/check_overlaps.py:7) and [scripts/check_hook_fit.py](/J:/code/marengo/hardware/designs/torso-frame/review/packet3/scripts/check_hook_fit.py:14) read `../generator/parts.json` and `*.stl` beside the scripts, neither of which exists in the packet layout (`data/frame/`, `data/cradle/`); [scripts/render_cradle.py](/J:/code/marengo/hardware/designs/torso-frame/review/packet3/scripts/render_cradle.py:25) renders only extrusions plus the pack, so [views/CRADLE_ASSEMBLED.png](/J:/code/marengo/hardware/designs/torso-frame/review/packet3/views/CRADLE_ASSEMBLED.png) shows no BMS, cables, shell, bracket heads, or board.

### Minors

- Cradle side volume: README 55,370 mm³ vs STL 55,434 mm³ (0.1 %); harmless, but state which is source of truth.
- Left detent pin is built differently in code (Ø3.05 with 30° draft vs Ø4.0 right, [scripts/build_cradle.py](/J:/code/marengo/hardware/designs/torso-frame/review/packet3/scripts/build_cradle.py:186)); this asymmetry is undocumented — section both STLs before relying on equal retention.
- The carrier rear face also bears on the front lips (|X| 42–48, Z 37.5), not only on the four pads; preload can bend the 4 mm plate between pads and lips.
- M5×10 through 4 mm feet leaves ~6 mm toward the T-nut; nut engagement in the actual 2020 slot is unverified in the packet.
- Hook release needs a blade over the pack top while pulling sideways ([data/cradle/README.md](/J:/code/marengo/hardware/designs/torso-frame/review/packet3/data/cradle/README.md:73)); expect two hands and a slip risk next to the pack lid.

## The eight questions

**1. Load case.** Per direction (pack 21.6 N/g): down onto the ledges is strongest (ledge `S = 211 mm³` per side, ≈4,200 N ideal, >50 g before the feet/rails govern). Forward into the lip and feet is next (order >20 g; feet bearing ≈1,000 N per M5 screw). Sideways fails at the windowed web at roughly 4–10 g (M2). Rearward depends on the untested thumbscrew bosses and pack-case inserts (B3). **Upward fails first overall: hooks at ~2 g (B1).** The pack therefore does **not** stay captured in a tip-over; after the hooks tear it can lift, shift, and drop onto the thumbscrews/BMS. PETG fragments plus a loose 2.2 kg pack also create the puncture risk the load case forbids ([PACKET.md](/J:/code/marengo/hardware/designs/torso-frame/review/packet3/PACKET.md:15)).

**2. 4.7 mm strip.** Adequate only in ideal uniform tension; not adequate as the sole top-foot link under peel, sharp corners, and creep (M1). Smallest fix: outer rib over the slot plus ≥2 mm slot-end radii.

**3. Thumbscrew bowing.** Bowing itself is low-stress, but it consumes the 0.5 mm float, makes clamp force spring-limited then insert-limited, and creeps into rattle (M3). Needs a datum-plus-spring scheme and a torque/shim procedure, not “tighten both.”

**4. Snap hooks.** Tongue math verified (0.83 %, 2.9 N), and the side-insertion path is box-clear of the frame. But hold relies on a 1 mm barb land, 0.4 mm worst-case detent, and 0.2–0.4 mm clearances that FDM tolerance and bridge sag can erase; release is fiddly (M4). Print a coupon, not the full strap set, first.

**5. Board carrier.** Static board load is trivial (250 g; 20 g ≈ 49 N over six standoffs), but the plate fails at its own edges (B2), standoff screws engage 2 mm (M5), and insert pads are thin. Interferences the author understated: countersink rims sit under the side cable band (|X| 50–55, Z 42–50) with only 0.5 mm from the carrier front face (Z 41.5) to the band; board-corner components taller than ~7.5 mm will meet the bracket heads at X ±46.5–51, Y 175–203 / 325–353, Z 55–65 even though the carrier plate itself clears them in Z; standoff heads are trapped under the pads after assembly. See [views/CARRIER.png](/J:/code/marengo/hardware/designs/torso-frame/review/packet3/views/CARRIER.png) — the drawing shows the corner holes but not these edge distances.

**6. Thermal.** Real risk is creep/softening and loss of retention at 36 W sustained, not melting (M6). No print decision should be treated as a run decision until temperatures are measured.

**7. Print orientations/settings.** Sides: mechanically sensible layer direction, practically unstable as specified (B4). Hooks on the rear face put tongue bending in-layer, which is right, but the 21.7 mm bridge sag lands directly in the 0.4–0.5 mm clearances. Carrier flat at 100 % infill will print, but expect corner warp on a 110 × 150 mm plate and sag in the downward Ø6.5 counterbores that are supposed to stay flush on the pads.

**8. Contradictions between files.** (a) The spec's continuous top strap (X −51 to 51, [data/cradle/CRADLE_SPEC.md](/J:/code/marengo/hardware/designs/torso-frame/review/packet3/data/cradle/CRADLE_SPEC.md:39)) is now two unequal hooks — the packet text still says “the strap hooks stop it lifting” as if it were one strap. (b) Cradle README says the insulating sheet “must narrow to |X| ≤ 42” ([data/cradle/README.md](/J:/code/marengo/hardware/designs/torso-frame/review/packet3/data/cradle/README.md:46)), while [data/frame/parts.json](/J:/code/marengo/hardware/designs/torso-frame/review/packet3/data/frame/parts.json) already has `pdb_sheet` at |X| ≤ 42 — leaving 0 mm to the lip face at X 42, not clearance. (c) `parts.json` still lists the replaced `packrail_*` webs and plain-box `cradle_strap` without barb/lip, by the author's own admission. (d) Frame text still describes two coplanar PCBs in one section and the single rev B 100 × 150 board in another; the carrier follows rev B. (e) Side volume differs by 64 mm³ between README and STL. (f) The left detent pin geometry differs in code without a note.

**Verdict: NO-GO for printing a first test set.**
