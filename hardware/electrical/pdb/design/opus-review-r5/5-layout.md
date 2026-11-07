# Rev B power board: layout and board-design review (lens 5)

2026-11-07. This review covers the floorplan and the layout rules before placement. Nothing is routed. The repo was read only.

**Sources read:** `pdb/design/{PRE_LAYOUT_AUDIT, LAYOUT_NOTES, README, RATIONALE-log, REVB*_REVIEW, GATE3_REVIEW}.md`, `FLOORPLAN_REVB.png`, `FLOORPLAN_REAL.png`, `layout/hp_fit_front.png`, `schematics/marengo_pdu_revb/tools/{floorplan,area,check,absorber_sim}.py`, `gen/gen.py` (the netlist source), `pdb/REV_B_PARTS.md`, `designs/torso-frame/README.md`, and `torso-frame/carrier/README.md`.

**Calculations:** `/tmp/claude-0/-home-user-marengo/b65a1c2d-e1c4-5a08-988b-8186ccee9383/scratchpad/calc5.py` (Python). I couldn't run `area.py` (no pcbnew) or `floorplan.py` (no matplotlib), so block sizes come from the constants in `floorplan.py` and from the PNG.

**Caveat on LAYOUT_NOTES.md.** It still describes rev A (the HP/LP pair, a 60 V `.kicad_dru`, 0.6 mm outer and 0.2 mm inner). Rev B has no layout rule file of its own yet, so every rule below is a proposal.

## Summary

| # | Sev | Finding | Conf |
|---|---|---|---|
| 1 | **Blocker** | C45 cannot go on the back face: there's 6 mm to the carrier, and the can is 12.6–13 mm tall | High |
| 2 | **Blocker** | The FET copper spec can't be met in the floorplan, and it was sized for 60 A, not 80 A | High |
| 3 | **Major** | The layer plan has no room for the 80 A GND return. 1 oz inner layers are too thin | High |
| 4 | **Major** | BAT+ and BAT− are on opposite side edges: wrong for the frame's cable path and for the pack-loop inductance | High |
| 5 | **Major** | GND stud assignment crosses the board, and no limb has its + and − together | High |
| 6 | **Major** | The main path doubles back: BAT → RS1 (in B's block) → bank A → back to bank B | High |
| 7 | **Major** | No stray-inductance budget for the commutation/absorber loop. FET margin is 9 V | Medium |
| 8 | **Major** | The terminal current rating is unverified, and the THR joints into heavy planes can't be hand-soldered | High / Med |
| 9 | **Major** | E-stop safety nets now sit next to 54 V nets in the top strip, with no separation rule | Medium |
| 10 | **Major** | Airflow: the only exhaust is the side band, which the terminal walls and pigtails block | Medium |
| 11 | Minor | Kelvin routing for RS1: one mm of mis-route costs 8.8 % of the current limit | High |
| 12 | Minor | C38 must stay out of the fan-zone height limit | High |
| 13 | Minor | Bring-up test points: no GND TP, and nothing scope-grade for the µs events | High |
| 14 | Minor | Gate-loop and clamp-loop layout rules | Medium |
| 15 | Minor | The 150 A / 100 ms requirement can't be reached (U1 latches first). Documents conflict | High |
| 16 | Note | Board size, layer count, split or merge | Medium |

---

## 1. BLOCKER: C45 does not fit on the back face

**Evidence**
- The board's rear face is at Z 47.5, and the carrier's front face is at Z 41.5. That's a 6 mm gap (`carrier/README.md`; standoffs are 6 mm F-F).
- C45 is CP_Elec_10x12.6 (APZ1012470M100R): a 10 mm can, 12.6 mm tall (13 mm in the audit's options table). It doesn't fit standing up, and it doesn't fit lying down (10 mm against 6 mm).
- **Through the carrier's lightening window** (X ±38, Y 273–328, R6 corners): from the board's rear (Z 47.5) to the top of the Nomex on the pack lid (Z 34.5) is 13.0 mm. That leaves 0–0.4 mm for the can, before the solder fillet, the pack's tolerance and vibration.
- The floorplan box for C45 is X −36.5 to −25.1, **Y 268.5–281.5**. The window starts at Y 273, so the can's lower 4.5 mm lands on solid PETG anyway.
- Rev A already had this rule (FLOORPLAN_REAL: "Tall or through-hole parts must stay on the front, since the rear has only 6 mm"). Fable flagged it in REVB2 ("6 mm standoffs, the pack behind, no airflow").
- C45 would also share that gap with the −X brake-resistor pair ("crossing under the board", REV_B_PARTS) and the standoff screw heads.
- The thermal problem is moot, but for the record: C45 would sit on the mirrored bank-B drain pour. The FET bench limit is a 95 °C case, in still air. Life of a hybrid polymer roughly halves for every 10 K. APZ's rated temperature and life aren't in the repo; read them before any hot placement.

**Fix.** Keep all three absorber cans on the front face, and place them as a cluster at the BAT entry (finding 4's re-floorplan makes room there).
- If the floorplan stays as it is, the 8 cm² free zone at the bottom works electrically. Can ESR (50 mΩ) dominates current sharing between cans. A 90 mm ABS/ABS_N run of 5 mm 2 oz copper each way is about 2 × 0.246 mΩ/sq × 18 sq ≈ 8.9 mΩ, so the far can takes about 15 % less of the pulse. That's acceptable, but it's second best.
- The board stays one-sided, which also keeps hot-plate reflow possible (finding 8).

**Confidence:** high. The stack numbers come from the frame and carrier docs.

## 2. BLOCKER: the FET thermal-copper spec is infeasible, and it's for the wrong current

**Evidence: area**
- The spec asks for ≥ 10 cm² of top-layer drain copper per FET: 4 × 10 = **40 cm²** on L1 for banks A and B (SW_D ≥ 20, MID ≥ 20).
- Row 2, which holds both banks (Y 268–302, X −36.5 to 24.3), is **20.7 cm² in total**. That includes Q1, Q2, Q8, Q9, U1, U8, RS1, C38 and the gate parts (14.4 + 6.2 cm² of blocks).
- Placement is at 88–90 % of the 60 % budget, so courtyards cover about 53 % of the face. Free L1 copper on the whole board is roughly 70 cm², and it also has to carry BAT, VBUS, GND and all routing. 40 cm² of it can't be set aside for drain pours next to the FETs.

**Evidence: current**
- The 2.7 W per FET and the RthCA ≤ 22 K/W budget were computed at a **60 A** bus. The board requirement is **80 A** continuous.

| Bus | Per FET (1.7 mΩ × 1.75 hot) | 4 FETs | RS1 (0.5 mΩ) | Tj at 22 K/W, 50 °C ambient | RthCA needed for 110 °C |
|---|---|---|---|---|---|
| 60 A | 2.68 W | 10.7 W | 1.8 W | 110 °C | 22.0 K/W |
| 80 A | 4.76 W | 19.0 W | 3.2 W | **157 °C** | **12.2 K/W** |

At 80 A, the board dissipates about 25–30 W in a closed torso: 19 W in the FETs, 3.2 W in RS1, and about 2 W each in the positive and GND copper (finding 3).

**Fix**
- **Decide the continuous rating the layout must meet.** I'd suggest 60 A continuous and 80 A for a bounded time (state the time), and write it into the layout spec.
- **Replace the "10 cm² on top" rule with a stack rule**:
  - L1 holds the tab pads, plus all the drain pour that fits.
  - L3 and L4 carry the mirrored drain pours. The board is one-sided, so the whole bottom is free copper.
  - Thermal vias under the tabs (30 × 0.3 mm ≈ 5.9 K/W, assuming 25 µm plating), plus more vias around them.
- Check the result with a quick FEM or a coupon. If 80 A continuous is a real need, make it 3 FETs per bank, or give the banks the lower fan too.

**Confidence:** high on the arithmetic; the 22 K/W figure is taken from the audit.

## 3. MAJOR: the layer plan has no room for the GND return, and 1 oz inner is too thin

**Evidence: layers.** The audit says "the 60 A nets (BAT, SW_D, MID, VBUS) pour on all four layers" and "≥ 1 oz inner". That leaves the 80 A **GND** return (studs to BAT−) with no layer. It also leaves no continuous reference plane under the absorber and commutation loop (finding 7). Rev A specified 2 oz inner (README §1).

**Evidence: widths for 80 A on one layer** (calc5.py)

| Method | 2 oz outer | 1 oz inner | 2 oz inner |
|---|---|---|---|
| IPC-2221 ext/int, 20 K rise | 41.6 mm | 216 mm | 108 mm |
| IPC-2221, 30 K rise | 32.5 mm | 169 mm | 85 mm |

**Evidence: stack at 80 A** (copper at 70 °C, per 100 mm of path)

| Stack | Width | R per 100 mm | Loss at 80 A | Worst-layer rise (IPC-2152 fit) |
|---|---|---|---|---|
| 4 layers, 2/1/1/2 oz | 30 mm | 0.33 mΩ | 2.1 W | 37 K |
| 4 layers, 2/1/1/2 oz | 20 mm | 0.49 mΩ | 3.1 W | 65 K |
| 2 outer only, 2/2 oz | 30 mm | 0.49 mΩ | 3.1 W | over 100 K |
| 1 outer 2 oz + 1 inner 1 oz | 30 mm | 0.65 mΩ | 4.2 W | off the chart |

On the IPC-2152 numbers:
- They use the common curve fit A = (117.555·ΔT^−0.913 + 1.15)·I^(0.84·ΔT^−0.018 + 1.159) to the 2152 baseline chart: no planes, still air.
- 80 A is beyond the chart's range, so the fit is extrapolated and pessimistic. Treat its rises as an upper bound, not a prediction.
- The physical estimate is: about 2 W per 100 mm of 30 mm path, over about 60 cm² of two-face area, at about 10–15 W/m²K in still air, gives roughly 20–30 K on top of the FET heat. Fan air cuts that substantially.

**Evidence: GND return**
- Stud 2 (X −43, Y 226) to BAT− (X +43, Y 291) is about 108 mm, taken here as 50 mm wide.
- On one 1 oz layer: 1.06 mΩ, 85 mV and 6.8 W at 80 A.
- On two 1 oz layers: 0.53 mΩ.
- On 2 oz + 1 oz + 1 oz: 0.27 mΩ.

**Fix**
- **Order 2 oz on all four layers.** Allocate:
  - L1: positive pours.
  - L2: solid GND, unbroken under the banks, the absorber and the bus caps.
  - L3: GND in the return corridor, positive (drain mirror) under the banks.
  - L4: positive mirror plus GND where the positive path isn't.
- Keep at least 30 mm of effective width for the main path on at least 3 layers, and at least 2 layers of GND return.
- Stitch with via arrays at every layer change: about 1 A per 0.3 mm via is a conservative planning figure; for 80 A use ≥ 80 vias per transition.

**Confidence:** high for the need. The exact rises are estimates.

## 4. MAJOR: BAT+ and BAT− on opposite side edges

**Evidence: cable path**
- The floorplan puts BAT+ at (X −43.4, Y 291.2) and BAT− at (X +43.4, Y 291.2), **87 mm apart**.
- The frame's reserved pack cable path comes over the **top edge** (X −5 to 25, Y 345–353.6; frame README "Reserved cable paths"). Rev A placed the pack lugs there on purpose (README §1a: "HP top-edge ring lugs and the 10 AWG bend stay inside Y 340 to 353.6").
- From the top edge, the 10 AWG leads would have to cross the front face over J24 or the HS50 A terminals, or use the side bands. The side bands are |X| 50–55, Z 42–50, 5 × 8 mm per side, "tight", and already hold an 8 × 3 mm arm bundle. A 10 AWG wire is bigger than the 12 AWG's Ø4 quoted there, so it doesn't fit.

**Evidence: inductance**
- Two 100 mm wires 87 mm apart are about **178 nH**. The same wires 5 mm apart are about **64 nH** (µ0/π·l·(ln(d/r)+¼), r = 1.3 mm).
- absorber_sim's 0.5–1 µH covers the whole pack loop. The split adds roughly 10–35 % of that for each 100 mm of separated run.
- On the board, the GND return from BAT− back to the absorber and banks also crosses the whole width.

**Fix.** Move BAT+ and BAT− to the **top edge**, side by side, centred in X −5 to 25.
- Move J25/J26 (18 AWG, 2.84 A) to the side positions that frees up.
- The main power then runs monotonically down the centre: BAT (top) → absorber and RS1 → bank A → bank B (fan zone) → bus caps → meters → limb terminals.
- The GND return runs back up under it on L2, which minimises the loop area.
- This also fixes findings 1, 6 and part of 5 and 10.

**Confidence:** high.

## 5. MAJOR: GND studs cross the board, and limb pairs aren't adjacent

**Evidence: assignment**
- `gen.py`: "GND stud 1: L arm, R arm, waist" and "GND stud 2: aux, L leg, R leg".
- The floorplan puts stud 1 at +X (Y 225.8), stud 2 at −X, R arm+ at −X, and L leg+ at +X.
- So the R arm's return (and later the L leg's) crosses 87 mm of board, and its pigtail pair splits at the board.
- No limb has its + and − adjacent: arms are at Y 277.8, studs at Y 225.8. Each pigtail is therefore an open loop until the wires meet, and the 20–60 A limb return currents run diagonally under the meter and logic area.

**Evidence: current per lug**
- Each stud takes up to 3 lugs stacked on one M4.
- Each limb is fused at 20 A, with 60 A peaks (audit). A stud can carry the whole bus: up to the 97–123 A U1 limit for ≤ 0.4 ms, and 80 A continuous if the load concentrates on one side.
- BAT+ and BAT− carry 80 A continuous each.

**Fix**
- Assign the studs by side: stud +X takes L arm, waist and L leg; stud −X takes R arm, aux and R leg.
- Better: put each stud **next to** its + terminals, so every pigtail pair leaves twisted.
- Limit a stud to 2 lugs, or use a lug-stacking washer stack whose height is checked against the Z 67 ceiling.
- Change check.py's stud text to match.

**Confidence:** high, from the netlist text and the floorplan labels.

## 6. MAJOR: the main-path detour

**Evidence**
- The netlist runs BAT → RS1 → SW_D → Q8/Q9 (bank A) → MID → Q1/Q2 (bank B) → VBUS (check.py lines 1–3).
- The floorplan puts RS1 in the **bank B** block (X −36.5 to 5.9), next to BAT+, and bank A at X 5.9 to 24.3.
- So the current goes east 40–60 mm on SW_D to bank A, then back west 20–40 mm on MID to bank B.
- That means two 80 A pours fighting over one 34 mm row, extra I²R (about 0.3 mΩ per 100 mm on 4 layers, finding 3), and extra commutation-loop length.
- U8's sense pair runs from RS1 in B's block over to U8 in A's block.

**Fix.** Order the blocks in the direction of flow: RS1 → bank A → bank B → VBUS. Put U1 and U8 both within about 15 mm of RS1's sense pads. With finding 4 this becomes a vertical stack under the upper fan.

**Confidence:** high.

## 7. MAJOR: no stray-inductance budget for the commutation/absorber loop

**Evidence: margin**
- With three cans, the worst-case BAT is 80–91 V (absorber_sim corners). The FETs, U1, U8 and D7 are 100 V parts, so the margin is **9 V**.
- absorber_sim models no board stray between the absorber and the FETs.

**Evidence: the loop.** When the banks open, current moves from the FET path to D7. The on-board loop with changing current is:

> C38/C44/C45 → D7 → BAT → RS1 → SW_D → bank A → MID → bank B → VBUS → bus caps C10–C13 → GND → ABS_N → R53 → back to the caps

**Evidence: what each nH costs.** V = L·di/dt, for 900 A (the user's figure; gen.py says 350–890 A):

| Current fall time | Volts per 5 nH | Volts per 20 nH |
|---|---|---|
| 3 µs | 1.5 V | 6 V |
| 1 µs | 4.5 V | 18 V |
| 0.2 µs | 22 V | 90 V |

Bank A's off edge is about 2–3 µs through 22R. U1's breaker is 1.2 µs (REVB2). So 1–3 µs is the realistic range, and **≤ 10 nH** keeps the overshoot to about 3–9 V.

**Evidence: what sets the inductance**
- Plane continuity sets it, not millimetres. L1 over an L2 GND plane at about 0.2 mm is about 0.25 nH per square, so a 72 × 30 mm loop over a solid plane is under 1 nH.
- The real contributors are via transitions, the D7 and can ESL, R53's footprint, and any break in L2.
- If the caps are spread (C44 in the top strip, 55 mm from BAT; C38 in B's block; C45 on the back), ABS_N becomes an island net that has to reach R53 first.
- Copper heating is not a problem: 900 A for 10 µs in 5 mm × 70 µm gives an adiabatic rise of about 0.3 K.

**Fix**
- Write a rule: absorber cans, D7 and R53 sit within one cluster at most 25 mm across, at BAT.
- ABS and ABS_N are pours directly over unbroken L2 GND. R53 is ≥ 5 mm wide with ≥ 10 vias to L2 and L3.
- The bus caps sit within about 20 mm of bank B's source pins, so VBUS is held during commutation and the load-side wiring's energy doesn't swing VBUS negative across the FETs.
- After layout, extract the loop L (KiCad or a field solver) and add it to absorber_sim as a series L on the FET side.

**Confidence:** medium. The di/dt isn't measured; the margin arithmetic is solid.

## 8. MAJOR: terminal rating and assembly

**Evidence**
- The REDCUBE 74651194 current rating is "unverified" (RATIONALE-log lines 189 and 297). Every main-path lug carries 80 A continuous: BAT+, BAT−, and both studs (finding 5).
- Each 150 mm of 10 AWG at 80 A dissipates 3.15 W per conductor (3.28 mΩ/m), and that heat flows into the terminal at the board edge, in no airflow.
- The audit says "THR terminals are hand-soldered". A THR pin joined solidly to 2 oz planes on 4 layers is far beyond a normal iron, even more so with 13 of them. With thermal reliefs instead, the pin's current path through the spokes becomes the bottleneck.
- Each D2PAK-7 tab has 30 vias and sits on ≥ 20 cm² pours. That also needs whole-board heat.
- A back-face C45 would force a second reflow side (moot after finding 1).

**Fix**
- Get the Würth datasheet: rated current and derating at the board ambient.
- If it's under 80 A, use two terminals in parallel for BAT±, or a stud-plus-bus-bar.
- Use solid connections (no reliefs) on the main-path terminals.
- Assemble on a hot plate or in an oven at least the size of the board (150 × 100), with paste-in-hole for the THR terminals per Würth's reflow profile. Don't plan on hand soldering, and keep the board one-sided.
- For the thermal vias, fill and cap them (VIPPO), or keep them out of the paste aperture, to avoid solder wicking under the tab.

**Confidence:** high on the assembly; medium on the rating (unknown, not known to be bad).

## 9. MAJOR: E-stop safety nets next to 54 V in the top strip

**Evidence**
- The latch block (RUN, RUN_A, RUN_B, UVLO, EN_A, FLT_A, LOOP_*, RST_*, MID_B, VB_B) moved to the top strip.
- Its neighbours there: the converter soft start (BAT, BRK_IN, SH9_*), C44 (ABS, about 54 V), the clamp, and J26 (VBUS/CL_D).
- **Leakage threshold.** When RUN is low, RUN_B floats behind its BAV19W, so UVLO's only sink is 33 kΩ. Lifting UVLO to 3.8 V takes 0.115 mA. From 54.6 V that's a leakage of **≤ 0.44 MΩ** from any BAT or VBUS copper into UVLO, and bank B enables without RUN. Flux residue, conductive debris or a hairline short in an inner layer can reach that.
- IPC-2221B gives 0.6 mm external and 0.1 mm internal at 51–100 V, and the rev A rule file had 0.2 mm inner. Those cover breakdown, not this failure mode.
- The U1 pin 2–3 (SH_P–UVLO) bridge is inherent to the MSOP pinout and was already accepted with the two-bank design. The board-level clearances are what's left to control.

**Fix**
- Make a net class "ESTOP_SAFE" for the nets above. Give it ≥ 1.5 mm (2 mm preferred) to any BAT, SW_D, MID, VBUS, ABS, CL_D, BRK_* or SH* copper on **all** layers, with a GND guard trace or pour between them.
- Run no 54 V plane on L2–L4 under the latch block.
- Clean the flux after assembly; consider conformal coating over U1, U8 and the latch.
- Keep J20 (the loop connector) at the far side of the block from the soft start.

**Confidence:** medium. The mechanism is real; the probability is low but the consequence is a defeated E-stop channel.

## 10. MAJOR: airflow against placement

**Evidence: air path**
- The intakes are 40 mm fans at |X| ≤ 20, Y 264–304 and Y 193–233, Z 70–80, blowing onto the front face (frame item 30: "2 intakes lower front").
- The lower box has no exhaust (frame gate 2). The proposed exit is the V1 side grilles, through Z 37.5–50 beside the board: the same 5 × 8 mm side cable bands.
- Both side edges are lined by 13.16 mm terminal cubes from Y 219 to 325 (−X column gaps: 0.24 mm, 12 mm with a standoff in it, 0.24 mm, 0.24 mm), with lugs and pigtails outside them. So the exhaust path is mostly walled off.

**Evidence: what each fan reaches**
- Upper fan: banks A and B, if the FETs sit at |X| ≤ 20. The bank B block runs to X −36.5.
- Lower fan: the MID meter and 8 cm² of empty space. **No heat source.** The 12 V brick has left the board.
- RS1 (1.8–3.2 W), the limb shunts (3.6 W peak each) and the BAT± terminals (lead heat) are outside both fan zones.

**Fix**
- Place all 4 FETs inside |X| ≤ 20, Y 264–304.
- Put RS1 and the busiest limb shunts under or next to a fan.
- Leave an air channel from each fan zone out to an edge: the top edge (13.6 mm to the rail) or the bottom edge (35 mm to the waist plate). Finding 4's re-floorplan opens the top-centre and the sides.
- Agree the exhaust with the frame and shell threads before placement, and at bring-up measure the case temperatures at 60 A for 10 min with the shell on.

**Confidence:** medium, because airflow isn't simulated.

## 11. Minor: Kelvin sense routing for RS1 and the limb shunts

**Evidence: error per mm.** Each mm of 2 oz copper inside the sense loop adds 0.246 mΩ/sq × 1/W:

| Copper width | Added R per mm | Share of RS1 (0.5 mΩ) | Share of a limb shunt (1 mΩ) |
|---|---|---|---|
| 5.6 mm (the pad width) | 44 µΩ | **8.8 %** | 4.4 % |
| 10 mm | 25 µΩ | 4.9 % | 2.5 % |

For example, U1's 97 A limit becomes about 89 A, under the "keep peaks under 90 A" software rule.

**Evidence: two consumers.** SH_P and SH_N feed U1 pins 1–2 and also U8's CS+/CS− (through 0R), U8's VS (through 100R) and a 100 nF to GND.

**Evidence: pad pinch.** RS1's current pads are 5.6 mm wide and on top only. A long 5.6 mm 2 oz trace would carry about 19 A at 20 K (IPC-2221 external), so 80 A at the pads relies on short length and heat spreading. That makes a local hot spot beside a 3.2 W part.

**Fix**
- Route the sense pair from the inner edge of the sense pads, as the audit notes (the Bourns L-tab is missing).
- Route it as a tight differential pair over GND, and branch only at the ICs.
- Flare the current pours across the full pad width. Put via arrays at the pad edge, not in the pad.
- Check RS1 by IR at 60 A.

**Confidence:** high.

## 12. Minor: C38 and the fan-zone ceiling

**Evidence.** In the fan footprints (|X| ≤ 20), the ceiling is Z 61.5, and the top copper is at Z 49.1. A 12.6–13 mm can tops out at Z 61.7–62.1. C38 is inside the bank B block, which crosses X −20 to 5.9 inside the fan zone.

**Fix.** Put C38 at X < −20 or at Y > 304, or let finding 4's cluster solve it. C44 (Y 309–322) and the bus caps (Y 238–250, the Z 67 strip) are fine.

**Confidence:** high.

## 13. Minor: test points for bring-up

**Evidence**
- `gen.py` has TPs only on VCTL, RUN, VBUS, +12V, BRK_IN and MID. **No GND TP.**
- Bring-up steps 1–3 scope µs events: BAT, MID and the absorber at 300–900 A. A Keystone 5015 loop with a ground clip picks up tens of volts at that di/dt.
- Step 2 wants the close current, but R53 is a copper NetTie with no way to measure through it.
- REVB2 wants FLT tested at bring-up, and close order needs the GATE and GATE_A waveforms.

**Fix**
- Add ≥ 3 GND TPs (one per quadrant).
- Add scope ground-spring pad pairs (signal plus GND within 3 mm) at BAT, SW_D, MID, VBUS and ABS–ABS_N.
- Add small pads on GATE, GATE_A, UVLO, FLT_A, BST_A, CL_G, BRK_T and BRK_PG.
- Give the R53 site a removable wire-loop option for a current probe or Rogowski coil, and Kelvin pads across RS1 for calibration.

**Confidence:** high.

## 14. Minor: gate-loop and clamp-loop rules

**Gate loops**
- U1 drives Q1 and Q2 through 10R from GATE, referenced to OUT = VBUS. Its fast breaker pulls 230 mA in 1.2 µs.
- U8 drives Q8 and Q9 through G1PD → 22R → GATE_A → 10R, with its source pin on MID.
- The D2PAK-7 has no Kelvin source pin, so any shared main-current copper in the gate return feeds back 0.45 V per nH at 450 A/µs.

**Gate-loop fix**
- Keep the driver within about 10 mm of its FETs.
- Put the 10R at the gate pin.
- Take the gate return as its own trace from the source pin nearest the gate (pin 2), running beside the gate trace, not from the pour.
- Keep the gate loops symmetric between the two FETs of a bank.

**Clamp loop**
- Q6 and D4 commutate 5.7 A in about 0.3 µs, 19 A/µs. The nearest VBUS–GND bulk is the bus caps, about 70 mm away, so the loop is about 70 nH. That gives about 1.3 V of overshoot: benign.

**Clamp-loop fix.** Add a 1 µF 100 V X7R across VBUS–GND at Q6 and D4 anyway: one footprint, low risk.

**Confidence:** medium.

## 15. Minor: the 150 A / 100 ms case, and document conflicts

**Evidence: 150 A**
- REV_B_PARTS says U1 latches after more than 97 A for 147–383 µs, and asks software to keep peaks under 90 A. So 150 A for 100 ms is not a reachable state.
- The copper pulse is trivial anyway: 150 A for 100 ms through 30 mm of 4-layer copper heats it about 0.3 K adiabatically.
- RS1 sees 7.6 W at 123 A for ≤ 0.4 ms. That's fine, but the CSS4J's short-time overload figure isn't in the repo.

**Fix (150 A).** Size the layout for continuous current (finding 2), and drop 150 A / 100 ms from the layout spec, or reconcile it with U1's settings.

**Evidence: document conflicts**
- The frame README says the brake-resistor leads "run down ... to the board's **bottom** edge". The board documents say the **top** edge.
- The −X brake pair "crosses under the board", through the 6 mm gap (finding 1).
- LAYOUT_NOTES.md is still rev A.

**Fix (documents).** Write a rev B LAYOUT_RULES (net classes, clearances, stack-up, widths, via counts), and align the frame and board documents on the lead routes.

**Confidence:** high.

## 16. Note: board size, layer count, split or merge

- **Size.** 100 × 150 is fixed by the carrier, the posts and the bay. It isn't wrong, but it's full. The heat and pour demands (findings 2 and 3) are what's short, not part area.
- **Layers.** Stay at 4 layers, but **2 oz on all four** (finding 3). 6 layers (two more GND/power layers) is the step after that, if the extracted loop inductance or the GND drop can't be met. Check the fab's 2 oz inner-layer option and price; I didn't verify it.
- **Split or merge.** Don't split the main path: the absorber, banks, RS1 and bus caps must share one plane pair. If the floorplan won't close once the pours are reserved, the low-risk area lever is to move the compute-side 12 V block (outputs, Pi header, 330 µF, 11.3 cm²) and the spare eFuses (6.4 cm²) to a small 2-layer board near the converter or Pi. Those carry ≤ 12 A and no E-stop function. Low confidence that this is needed: do the finding 4 re-floorplan first.

## Checked and OK

- **IPC-2221B clearances** at 54.6 V nominal and 80–91 V absorber peaks: B2 external is 0.6 mm and B1 internal is 0.1 mm for 51–100 V, and B2 stays 0.6 mm at 101–150 V for a D1 fault excursion. The rev A rule's 0.6 mm outer and 0.2 mm inner is electrically sufficient. Separation for safety nets is a separate rule (finding 9).
- **Pulse heating.** The 150 A / 100 ms main path rises about 0.28 K adiabatically. The ABS_N copper at 900 A for 10 µs rises about 0.3 K. R53 at ≥ 5 mm is fine thermally.
- **Ground offsets for logic.** The return-current drop is 21–85 mV, depending on how many GND layers there are. I2C to the Pi and the INA228 single-ended VBUS/MID readings are unaffected at that level. The shunt readings are differential. There's no CAN on the rev B board, so there's no CAN common-mode issue on the board. Keeping the return out of the logic area is still good practice (findings 4 and 5).
- **Thermal vias.** 30 × 0.3 mm at 1 mm pitch gives about 5.9 K/W, assuming 25 µm plating, and fits under a D2PAK-7 tab. The count is adequate; the fill is a separate issue (finding 8).
- **Corner keep-outs and standoffs.** Terminals and J24 clear the |X| > 46, Y 190–203 and 325–340 corner zones. The (±46, 265) M3 head (Ø about 5.7) has about 3 mm to the adjacent terminal courtyards. That's fine electrically; fix the lug orientation so a lug can't rotate into the head.
- **Bus caps** (Y 238–250) are in the Z 67 strip. C44 is outside the fan zone. The J25/J26 top-edge lugs have 13.6 mm to the front rail for an 18 AWG turn.
- **Clamp loop inductance** is benign (finding 14).
- **The pad matches** in the audit (CSS4J on the BVR land, TO-263-7 tab, TDSON-8 drain pads) agree with what layout needs. The note about routing from the sense-pad inner edge stands.
