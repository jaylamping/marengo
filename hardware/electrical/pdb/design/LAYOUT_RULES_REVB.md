# Power board rev B: layout rules

2026-11-07, rev B5. These rules come from the Opus round 5 layout review ([opus-review-r5/5-layout.md](opus-review-r5/5-layout.md)) and Joey's single-sided rule. They replace LAYOUT_NOTES.md, which still describes rev A. tools/place.py follows them.

## Board

- 100 × 150 mm, set by the carrier. M3 holes at X ±46, Y 194 / 265 / 336.
- **Single-sided: every part on the front face.** The rear has 6 mm to the carrier, and a back-side part would need a second reflow pass (Joey, 2026-11-07).
- 4 layers, 1.6 mm, **2 oz outer / 1 oz inner** (JLC's standard stackup; rev B5 thermal option D takes the FET heat into a heatsink, so the copper only carries current).
  - L1: positive pours and parts.
  - L2: solid GND, unbroken under the banks, the absorber, the bus caps and the port FETs.
  - L3: GND in the return corridor (the second GND return plane), signals where needed.
  - L4: positive mirror, with GND wherever the positive path isn't.
- Rating to lay out for: **60 A continuous, under 90 A peak.** Keep at least 30 mm of effective main-path width on at least 3 layers, and at least 2 layers of GND return. At layer changes, use about 1 A per 0.3 mm via: 60 or more vias per main-path transition.

## Power path

- **Flow order down the board:** BAT pair (top edge) → absorber cluster and RS1 → bank A → bank B (under the upper fan) → bus caps → port breakers → limb pigtails. Remove the old B → A → B detour.
- **Pack entry:** the BAT and GND 8 AWG pair (4.5 mm holes, 8 mm pads, 14 mm pitch; to an XT90) sits side by side on the top edge, in the frame's pack cable path. **Keep-out: no components in X −5..25, Y 315–340** (the leads run up the face and bend over the top edge at R15 from about Y 330); the pads sit in that strip. Solid connections to all four layers, no thermal reliefs. A cable-tie hole each side of the pair.
- **Absorber cluster:**
  - D7, D21 and the eight cans in one cluster at the BAT entry; R53 (the ABS_N to GND link) under the can stack.
  - ABS and ABS_N are pours directly over unbroken L2.
  - R53 is at least 5 mm wide, with 10 or more vias to L2 and L3.
  - **Loop budget about 2 nH**, not 10 (rev B5 thermal study: with a 50–200 ns bank turn-off, 5 nH gives BAT 88–102 V
    and 10 nH gives 102–111 V without drain ceramics). Extract it after layout and run it in absorber_sim.
- **Bank A drain ceramics C87/C88** (2 × 2.2 µF 100 V 1210, SW_D to GND): right beside RS1's SW_D pad and the bank A tabs,
  GND pads straight down to L2 with 4 or more vias each. They hold BAT at 74 V and the drain under 85 V.
- **FET copper and the heatsink (option D, [THERMAL_OPTION.md](THERMAL_OPTION.md)):**
  - 2.7 W per FET at 60 A goes down into a finned heatsink under the board (X −33..33, Y 200–305).
  - About 50 × 0.3 mm vias per tab, at the pad edge and under it, tented on L4 and kept out of the paste aperture so they
    don't wick solder on the hot plate.
  - L4: bare copper (no mask, ENIG) 15 × 15 mm under each FET, for the insulating gap pad. Nothing else on L4 inside
    the heatsink zone except those pads and GND fill.
  - Heatsink holes HS1 (0, 286) and HS2 (0, 204), M3, no copper within 2 mm; the MID pour flows around HS1.
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
- **D2PAK-7 drain lead:** pin 4 (the centre lead) and the tab are the same drain but separate pads; a 0.5 mm track joins
  them under the package body (it keeps 0.6 mm to pins 3 and 5). Pin 4 splits the source pins, so pins 2–3 reach the
  source copper through vias at their outer ends into the L4 mirror. Pins 5–7 carry the source current on L1; three of
  the five source leads are plenty for 30 A.
- **Left limb ports** are the right ports' power row turned 180° (shunt, FET, output clamp, pigtail, meter), so every port
  FET has its gate pin on the inner side and its source pins facing the pigtail.
- **Brake headers J3/J25:** VBUS and CL_D run up the can column on L4 as 2 mm tracks (X 34.5 and 40.5).
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
