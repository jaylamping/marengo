# Custom 21700 pack for the torso

> **Update 2026-11-06:** the build is now planned in [`pack-build-plan.md`](pack-build-plan.md), which wins where the two differ.
> Changed there: cells are **P50B** (P45B sold out, P50B is 60 A / 12.8 mΩ); **no BMS on the pack** (BQ76952-class high-side BMS on the
> power board, a JK only for bench bring-up), so the BMS-on-the-pack-rear-face and 2 A active balance lines below are superseded;
> **charge to 4.15 V/cell (53.9 V)** through the board's XT60 charge port; series links are **wide 0.2 mm nickel plates**, not 0.2 × 10 strip;
> balance leads go through a fused pack sense board to one locking Micro-Fit connector instead of a JST plug.

2026-11-06. Answer to Joey's "dig into the custom 21700 option". Torso coords as in `hardware/designs/torso-frame/README.md`.
Numbers marked (verify) come from memory of the datasheets and need a check against the current sheet before ordering.

## The short version

A **13S2P pack of high-drain 21700 cells** gives the RS03s their rated 48 V, carries about 470 Wh, weighs about 2.2 kg,
costs about $300 in parts (about $400 with a spot welder), and **fits the rev A lower box as already specced (150 wide,
bottom Y 155) with no frame change.** Two DeWalt DCB6112 packs give 432 Wh at 36 V, weigh 3.0 kg, cost about $730 with a
charger, and force the lower box to 190 wide or 90 mm taller. The custom pack is less money and less frame; what it costs is
a day of careful building and the fact that you are your own safety certification.

## 1. Voltage: 13S, not 14S or 10S

| Series count | Nominal | Full charge | Empty (3.0 V/cell) | Against the RS03 (24 to 60 V, rated 48 V) |
| --- | --- | --- | --- | --- |
| 10S (what two FlexVolts in 20 V mode give) | 36.0 V | 42.0 V | 30 V | Works. Motors run at 75% of rated speed; torque unchanged |
| **13S** | **46.8 V** | **54.6 V** | **39 V** | **Rated voltage, 5.4 V of headroom under the 60 V ceiling for regen spikes** |
| 14S | 50.4 V | 58.8 V | 42 V | Legal on paper, 1.2 V from the ceiling. A braking arm pushes the bus over 60 V. No |
| 15S (FlexVolt 60 V mode) | 54.0 V | 63.0 V | 45 V | Over the ceiling when charged. No |

13S is also the e-bike standard, so chargers, BMS boards and reference designs are everywhere.

## 2. Cells: high drain, not high capacity

The bench supply is 24 V at 50 A. Two shoulders plus the waist at peak can ask for 40 to 50 A on the bus. In 2P each cell
carries half, 20 to 25 A continuous with spikes above. That rules out the 10 A "energy" cells (Samsung 50E, LG M50).

| Cell | Capacity | Continuous current | Mass | Price each | Note |
| --- | --- | --- | --- | --- | --- |
| Molicel P45B | 4.5 Ah | 45 A (verify) | 70 g | $5 to 7 | The default choice for high-drain packs. 2P gives 90 A continuous |
| Molicel P50B | 5.0 Ah | 50 A class (verify) | 70 g | $7 to 9 | Newer, same footprint, more energy |
| Samsung 50S | 5.0 Ah | 25 A (35 A pulse) (verify) | 71 g | $5 to 6 | Enough at 2P (50 A), less headroom, runs warmer |

Pick P45B unless P50B is in stock at a sane price. Buy from a cell vendor that sells genuine matched cells (18650BatteryStore,
IMR Batteries, Liion Wholesale), never marketplace listings.

**13S2P with P45B: 26 cells, 9.0 Ah, 421 Wh. With P50B: 10 Ah, 468 Wh.** 2 kWh of FlexVolt costs the same as 1.3 kWh here per
watt-hour bought, roughly: FlexVolt about $0.70 per Wh after the 20 V-mode derate, custom about $0.35 per Wh.

Runtime at a 200 W average draw (two arms moving, compute on, no legs yet): about 2 h. The arm thread can refine the average
from the bench logs.

## 3. It fits the rev A lower box

A 21700 is 21.7 mm across and 70.5 mm long. Lay the cells with their axis along Z (depth) in a 4 wide by 7 tall grid, 28
positions, 26 used; the two spare positions at a top corner take the fuse and the balance-lead header.

| | Pack | Rev A lower box clear space | Margin |
| --- | --- | --- | --- |
| X (width) | 4 × 21.7 + 3 × 1 gap + 2 × 2 wall = **95** | 110 between the posts | 7.5 per side |
| Y (height) | 7 × 21.7 + 6 × 1 + 2 × 2 = **162** | 178.6 (bottom rails to shared rails) | 16, or 8 top and bottom |
| Z (depth) | 70.5 + 2 × 4 (nickel, fish paper, lid) = **79** | 100 between the front and rear rail planes | 21 |

So the pack sits at |X| ≤ 47.5, Y 183 to 345, Z −45 to +34, with **24 mm clear in front of it on the frame centreline for the
power board** standing against the front rail plane (Z 34 to 58, poking 0 mm past the frame face if the board is 24 thick) and
the BMS (60 × 40 × 8 class) on the pack's rear face inside the Z margin. Nothing about the frame changes: bottom stays Y 155,
width 150, the rev A cut list stands, and the lower shell keeps the 156 waist. The pack mass (about 2.2 kg) sits at Y 264,
which is lower than the compute and about where the FlexVolts would have been.

Removal: the pack is a box on two printed rails, pulled straight out the back through D2. D2 is 116 wide, fine, but only 128
tall (Y 167 to 295); it needs to grow to about Y 175 to 350, which is the shell thread's call. Hot swap is a second pack and a
60 A connector (XT90S, which has an anti-spark resistor built in) behind an ideal-diode OR on the power board. For a first
build, power down, swap, power up is 60 seconds and skips the OR-ing.

A second, fixed cell in the pelvis later (Joey's groin idea) is the same recipe at whatever shape the pelvis allows.

## 4. Parts and cost

| Item | Qty | Cost | Note |
| --- | --- | --- | --- |
| Molicel P45B cells | 26 (+2 spares) | $150 to 190 | matched, same batch |
| 13S smart BMS, 60 A continuous, 2 A active balance, Bluetooth | 1 | $45 to 80 | JK or JBD. Common-port, with a charge FET. Verify the continuous rating is real (heatsinked) |
| Pure nickel strip 0.15 × 8 mm, and 0.2 × 10 for the series links | 2 m each | $15 | pure nickel, not nickel-plated steel |
| 21700 cell holders 4 × 7 (snap-together) | 2 sheets | $12 | they set the 1 mm gap |
| Fish paper rings and sheet, Kapton | | $10 | ring on every positive end |
| Fuse, MIDI 60 A, holder | 1 | $8 | on the pack, before the connector |
| XT90S pair, 10 AWG silicone, balance-lead JST | | $15 | |
| 13S charger, 54.6 V 5 A | 1 | $40 to 60 | through the BMS, attended |
| Case: printed in ABS or PC, or a folded 1 mm aluminium box with a printed liner | | $10 to 30 | not PETG next to cells |
| **Parts total** | | **$305 to 420** | |
| Spot welder, kWeld class (recommended) | 1 | $250 | or a $60 to 90 capacitor welder, which does 0.15 mm nickel fine |
| **With a cheap welder** | | **$370 to 510** | the welder is reusable for the pelvis pack |

Against that: two DCB6112 at $289 to 329 each plus a DCB118 charger at $130 to 150 is $710 to 810, and the frame rework.

## 5. Build, in order

1. Grade the cells: measure every cell's resting voltage and internal resistance, reject outliers, group into 13 pairs.
2. Dry-fit in the holders, mark polarity, fish-paper every positive end.
3. Parallel-weld the pairs first, then the series links. Four welds per cell end, pull-test a scrap strip first.
4. Balance leads: 14 wires, B0 to B13, in order, each fused on the BMS board or with a 1 A fuse in line.
5. BMS: configure 2.9 V cell cutoff, 4.15 V charge cutoff (gives up a little capacity for cycle life), 60 A discharge,
   60 °C cutoff, 2 A balance above 3.9 V.
6. Case, fuse, XT90S, label it.
7. First charge attended at 1 A, watch per-cell voltages on the app. Then a discharge test on the bench supply's dummy load.
8. Only then onto the robot, through the power board's precharge and E-stop.

Half a day for a first pack once the parts are on the bench.

## 6. The safety list, honestly

- The cells are the same ones DeWalt and Milwaukee use. What you lose is the pack-level engineering: moulded cell spacing, vent
  paths, UL 2595 testing. You replace that with holders, fish paper, a fuse, a BMS, and the power board's own protection.
- **The BMS is not the safety system.** Cheap BMS FETs can fail shorted. The pack fuse and the power board's contactor or E-stop
  are the layer that always works. Keep both.
- Spot-weld, never solder, to the cells. A soldered cell end is a cooked separator.
- Charge attended, through the BMS, in a metal tray. Store at 50% if it sits for weeks.
- A tip-over must not crush the pack: the frame puts 2020 posts on both sides and the gussets outboard, and the pack sits 25 mm
  inside the rear face. The crush pad work in the shell thread covers the back.
- Insurance and the household: a 470 Wh home-built lithium pack is a real fire source. Keep a sand bucket or a lithium
  extinguisher where it charges. This is the honest cost of the custom route.

## 7. Weight

Joey asked for weight to count. Pack-level figures, as they would be mounted:

| | Mass on the robot | Energy | Specific energy | Where the mass sits |
| --- | --- | --- | --- | --- |
| Two DCB6112 + two docks | 2.98 + 0.12 = **3.1 kg** | 432 Wh | 140 Wh/kg | Lower box, Y 88 to 348 (D3 tall) or the pelvis |
| 13S2P P45B, holders, nickel, BMS, case | 1.82 + 0.08 + 0.05 + 0.25 = **2.2 kg** | 421 Wh | 190 Wh/kg | Lower box, centre Y 264, inside the frame |
| 13S2P P50B, same build | **2.2 kg** | 468 Wh | 210 Wh/kg | same |
| 13S e-bike brick | 2.8 to 3.5 kg | 500 to 700 Wh | 170 to 200 Wh/kg | Does not fit the torso |

The custom pack saves about 0.9 kg against the FlexVolts for the same energy, and the saving is in the torso, above the hips,
which is where mass hurts balance and the tip-over case most. The torso frame rev A estimate was 2.3 kg of extrusion, brackets
and gussets; the pack is nearly a frame's worth of mass either way, so the 0.9 kg matters. The arm thread's tip-over numbers
scale with total mass, so every kilogram in the torso costs crush distance.

## 8. Comparison

| | Two DCB6112 (20 V mode, series) | 13S2P custom, P45B | 13S e-bike brick (500 Wh, Hailong or bare) |
| --- | --- | --- | --- |
| Bus voltage | 36 V nominal | 46.8 V nominal | 46.8 V nominal |
| Energy | 432 Wh | 421 to 468 Wh | 500 to 700 Wh |
| Mass | 3.1 kg with docks | 2.2 kg | 2.8 to 3.5 kg |
| Fits the rev A frame | No: 190 wide or +90 tall | **Yes, unchanged** | No: 370 × 90 × 110 class |
| Hot swap | Yes, built in | With a second pack and OR-ing | No |
| Safety | UL, moulded, BMS in the tool path | Yours to build | Unknown cells, usually no vendor you can reach |
| Cost with charger | $710 to 810 | $350 to 500 | $250 to 400 |
| Time | Order and print docks | Half a day build + a spot welder | Order |

## Recommendation

Build the 13S2P. It is the only option that keeps the frame and the slim waist exactly as reviewed, runs the motors at their
rated voltage, and costs the least. Keep the DCB6112 bay as a fallback in the generator (option D3 tall is modelled) in case
the first pack goes badly. Order of work: cells and BMS on order now; the power board gets designed around 13S (54.6 V in,
48 V bus, 12 V and 5 V rails, precharge, E-stop, XT90S in); the shell thread grows D2 to pass a 95 × 162 pack.
