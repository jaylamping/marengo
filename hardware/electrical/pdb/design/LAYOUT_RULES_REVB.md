# Power board rev B: layout rules

2026-11-07, rev B5. These rules come from the Opus round 5 layout review ([opus-review-r5/5-layout.md](opus-review-r5/5-layout.md)) and Joey's single-sided rule. They replace LAYOUT_NOTES.md, which still describes rev A. tools/place.py follows them.

## Board

- 100 × 150 mm, set by the carrier. M3 holes at X ±46, Y 194 / 265 / 336.
- **Single-sided: every part on the front face.** The rear has 6 mm to the carrier, and a back-side part would need a second reflow pass (Joey, 2026-11-07).
- 4 layers, **2 oz on all four**, 1.6 mm thick.
  - L1: positive pours and parts.
  - L2: solid GND, unbroken under the banks, the absorber, the bus caps and the port FETs.
  - L3: GND in the return corridor, plus a positive mirror under the banks.
  - L4: positive mirror, with GND wherever the positive path isn't.
- Rating to lay out for: **60 A continuous, under 90 A peak.** Keep at least 30 mm of effective main-path width on at least 3 layers, and at least 2 layers of GND return. At layer changes, use about 1 A per 0.3 mm via: 60 or more vias per main-path transition.

## Power path

- **Flow order down the board:** BAT pair (top edge) → absorber cluster and RS1 → bank A → bank B (under the upper fan) → bus caps → port breakers → limb pigtails. Remove the old B → A → B detour.
- **Pack entry:** the BAT and GND 8 AWG pair (4.5 mm holes, 8 mm pads, 14 mm pitch; to an AS150U) sits side by side on the top edge, in the frame's pack cable path (X −5 to 25). Solid connections to all four layers, no thermal reliefs. A cable-tie hole each side of the pair.
- **Absorber cluster:**
  - C38, C44, C45, D7 + its twin, and the R53 copper link all within 25 mm, at the BAT entry.
  - ABS and ABS_N are pours directly over unbroken L2.
  - R53 is at least 5 mm wide, with 10 or more vias to L2 and L3.
  - Loop budget is 10 nH or less: extract it after layout and add it to absorber_sim.
- **FET copper (2.7 W per FET at 60 A):**
  - L1 tab pads plus all the drain pour that fits; L3 and L4 carry mirrored drain pours.
  - 30 × 0.3 mm vias under each tab, filled and capped or kept out of the paste aperture.
  - Keep all four bank FETs inside the upper fan footprint (|X| ≤ 20, Y 264–304).
  - Check with a coupon or quick FEM; a clip-on heatsink per bank is the fallback.
- **Bus caps:**
  - within 20 mm of bank B's source pins and of the port FET drains;
  - C38 and the other 12.6 mm cans stay out of the fan-zone height limit (Z 61.5 ceiling at |X| ≤ 20).
- **Port breakers:**
  - each TPS48111 within 10 mm of its FET;
  - PD → 22 Ω → gate at the gate pin;
  - the 2.2 µF drain cap at the drain;
  - the SK810L output clamp at the source;
  - the shunt Kelvin pair runs to both the TPS48111 (VS, CS+ through 100 Ω, CS−) and the INA237.
- **Limb outputs:** each limb's + and return are on one pigtail pair at the board edge, twisted from the exit. Add a strain-relief tie hole beside each pair.
- **Brake headers J3/J25:** at the top edge. The −X pair runs above the board, not through the 6 mm gap behind it.

## Sensing and gates

- **RS1 and every shunt:**
  - Kelvin traces from the inner edge of the sense pads, as a tight pair over GND, branching only at the ICs.
  - Flare the current pours across the full pad width, with via arrays at the pad edge, not in the pad.
  - 1 mm of 5.6 mm-wide copper inside the sense loop costs 8.8 % of U1's limit.
- **Gate loops:**
  - each driver within 10 mm of its FETs;
  - the gate resistor at the gate pin;
  - the gate return taken as its own trace from the source pin nearest the gate, running beside the gate trace;
  - symmetric between the two FETs of a bank.

## E-stop safety nets

- Net class **ESTOP_SAFE:** RUN, RUN_A, RUN_B, RUN_P, UVLO, EN_A, FLT_A, FLT_D, LOOP_*, RST_*, L_B, L_E, L_PB, L_PC, MID_B, VB_B, INT_B, WDO_N, VWD.
- At least 1.5 mm (2 mm preferred) to any BAT, SW_D, MID, VBUS, ABS, CL_D, BRK_*, SH* or V_L* copper, on all layers, with a GND guard trace or pour between them.
- Why: 0.44 MΩ of leakage from 54 V into UVLO enables bank B by itself.
- No 54 V plane on L2–L4 under the latch block.
- Wash off the flux, and consider conformal coating over U1, U8 and the latch.
- J20 (the loop connector) sits on the far side of the latch block from the converter soft start.

## Clearance

IPC-2221B: 0.6 mm external, 0.2 mm internal for 51–100 V nets. The 0.15 mm intra-footprint gaps on U2 and U7 need a fine-pitch rule on those footprints only.

## Air

- Leave an air lane from each fan zone out to an edge (top or bottom). Keep the terminal-free edges clear.
- RS1 and the busiest port shunts sit under or beside a fan.
- At bring-up, measure case temperatures at 60 A for 10 min with the shell on.

## Test

- 3 GND test points, one per quadrant.
- Scope-ground pad pairs (signal plus GND within 3 mm) at BAT, SW_D, MID, VBUS and ABS–ABS_N.
- Pads on GATE, GATE_A, UVLO and FLT_A for the close-order test.
