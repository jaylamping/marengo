# 13S2P pack: build plan

2026-11-06. Thread "Build the 13S2P battery pack". This is how we actually source, build, charge and test the pack that
[`custom-21700-pack.md`](custom-21700-pack.md) chose. Where the two docs disagree, this one wins; the old doc carries a note
saying which of its lines changed.

Agreed with the power board thread ([`../pdb/PLAN.md`](../pdb/PLAN.md) section 3.1, main 5041df6) and the arm trunk thread
([`../wiring/left-arm-trunk-plan.md`](../wiring/left-arm-trunk-plan.md)).

Evidence labels: **datasheet** (read today from the maker's sheet), **vendor page** (read today, prices move),
**estimate** (my arithmetic), **verify** (from memory, check before ordering), **agreed** (settled between threads).

## The short version

- **Cells: Molicel P50B, not P45B.** It is 60 A continuous against 45 A, 12.8 mΩ DC resistance against 15 mΩ, and 10% more
  energy in the same can (datasheet). It is in stock today at $10.50 each for 10 or more; P45B is sold out at the cheaper
  vendor (vendor page). 28 cells (26 plus 2 spares) is about $295.
- **The BMS stays on the power board, high-side (agreed).** The pack itself has no BMS. It is cells, a 60 A MIDI fuse, an
  XT90S, 3 NTCs, and a small **pack sense board** that fuses the 14 cell taps and brings them out on one locking Micro-Fit
  connector. A cheap 13S JK BMS is the bench tool for grading, first charges and discharge tests before the power board exists.
- **Charge to 4.15 V per cell (53.9 V), not 4.2 (agreed).** This leaves room for regen in a full pack and adds cycle life
  for about 3% of capacity. Use an adjustable-voltage charger at 4 A, ending in an XT60.
- **Busbars are wide plates, not 10 mm strip.** A 10 mm nickel strip cannot carry 60 A. The pack doc's nickel line is
  replaced (section 4).
- **Tools to buy: a ~$40 handheld spot welder.** Joey has the meter; pre-crimped leads replace a crimper.
- **Pack total about $505 including the welder** (section 9). The BMS and charger (about $110) count against the power board.

## 1. What the pack is, and what it is not

```
 cells 13S2P (26 x P50B, wide Ni plates)
   B13+ ── 60 A MIDI fuse ── 10 AWG ── XT90S male (anti-spark) ──┐
   B0−  ─────────────────── 10 AWG ── XT90S ─────────────────────┤──► power board top edge, rear face (Y ≈ 335)
   B0..B13 taps (14) ─┐                                          │
   NTC1..NTC3 ────────┴─ pack sense board (fuse per tap) ── Micro-Fit 3.0 2x10 ─► board AFE (BQ76952 class)
```

- **On the pack:** cells, plates, fuse, XT90S, sense board, three 10 kΩ NTCs (103AT curve, B = 3435, the BQ76952 default; board thread verifies),
  printed case, foam, wrap.
- **Not on the pack:** charge and discharge FETs, balancing, coulomb counting, shunt. They live on the power board.
- **Consequence (agreed):** the pack charges in the robot through the board's charge port. A second swap-and-charge pack
  would need its own BMS. That is not planned; removing the pack is a service job.

### Pack fuse voltage rating (flag from the trunk and board threads)

Common automotive MIDI/ANL/blade fuses are rated **32 V DC**, which won't reliably break an arc at 54.6 V (57.5 V with the
regen clamp). The pack fuse and its holder must carry a DC rating above that:

- **Preferred: Class T 60 A ferrule fuse (Littelfuse JLLN060 class, 300 V AC / 160 V DC, verify)** in a matching DC-rated
  holder. That gives 2.8× voltage margin, very high interrupt rating, and the ferrule is small (about 14 × 22 mm, verify). It
  fits the spare grid positions.
- **Fallback: Littelfuse MIDI 58 V series** (60 A 58 V bolt-down part 142.5631.5602 seen at Digi-Key, verify series and
  holder rating). Only 0.5 V above the regen clamp, so it's a fallback.
- **Interrupt rating check (estimate):** a dead short at the pack is about 54.6 V ÷ 90 mΩ ≈ 600 A. Both options are rated
  far above that (verify on the datasheet).

### Why the BMS stays on the board (checked from the pack side)

The power board thread's reason is ground. A high-side BMS keeps pack negative as system ground, so a BMS trip can't lift
every CAN transceiver, the Pi and the drives by up to 54 V. The cheap JK/JBD boards switch the low side. I looked for a reason
to put it on the pack instead and found two, neither strong enough:

1. *A pack out of the robot has no protection.* Its only exposure is a short across the XT90 or the sense connector. The fuse
   covers the XT90. The sense board fuses cover the taps. Over-discharge from self-discharge takes years. The handling rules
   in section 7 cover the rest.
2. *Swap-and-charge.* Not needed for a robot that runs about 2 hours per charge on the bench (estimate, pack doc section 2).

### Where the pack mates (board thread, main 9ce67a0)

Both connectors mate at the **pack's rear top corner, behind the D2 door**. The board's leads (a ring-lugged 10 AWG pair and
the sense harness) run over the pack's top edge, which has 8.6 mm of headroom, to an XT90 and a 2 × 10 plug. On the pack
side: a short XT90S pigtail (about 60 mm, 10 AWG) from the fuse, and the sense board's header on the rear face at X 5 to 45,
Y 318 to 343. Both are parked in a printed holder on the case so nothing hangs loose when the door is open.

### Connector mating rule (agreed)

**Sense connector in first, XT90 last. XT90 out first, sense connector last.** The board thread is checking the BQ76952's
random cell attach claim and how fast it detects an open wire. The datasheet (section 16.3) tolerates random cell attach as long as
each tap lands on its own ascending VC pin. **The sense connector is a 20-pin Micro-Fit (2 × 10), agreed:** 14 taps,
3 NTCs, 2 pins for a connector-present loop that opens DSG directly if the sense connector is pulled, 1 spare.

## 2. Cells

| | Molicel P45B | **Molicel P50B** |
| --- | --- | --- |
| Capacity, typical / minimum | 4.5 / 4.3 Ah | **5.0 / 4.85 Ah** |
| Continuous discharge | 45 A | **60 A** |
| Standard / maximum charge | 4.5 A / 13.5 A | 5.0 A / 25 A |
| DC resistance (50% SOC) | 15 mΩ | **12.8 mΩ** |
| AC resistance (30% SOC) | 7 mΩ | 6.5 mΩ |
| Charge temperature | 0 to 60 °C | −20 to 60 °C |
| Mass, max | 70 g | 71 g |
| Size, max | 21.55 × 70.15 mm | same |
| Price today | $5.99 sale, **sold out** (IMR Batteries) | **$10.50 each at 10+, in stock** (18650BatteryStore) |

Sources (datasheet): [P45B](https://www.imrbatteries.com/content/molicel_p45b.pdf),
[P50B](https://www.fsaeonline.com/content/Molicel-TDS--INR-21700-P50B-80122.pdf).
Vendor pages: [P50B, 18650BatteryStore](https://www.18650batterystore.com/products/molicel-21700-p50b-5000mah-50a-battery),
[P45B, IMR Batteries](https://imrbatteries.com/products/molicel-p45b-21700-4500mah-45a-battery).

**Preferred if obtainable (Joey's find, Battery Mooch's 21700 table updated 2026-10-13):** Mooch's independent tests
put **Reliance RS60, Linkdata 60P and Great Power 60Q (CCC)** in his 30 A column at 16.9 to 18.7 Wh delivered per cell; the
P50B is not on his list. At 2P behind a 60 A fuse each cell must handle 30 A, so the 30 A column is ours. 6 Ah cells give
about 12 Ah / 560 Wh, about 20% more than P50B, in the same 21700 grid (check the bare-cell length is 70 to 71 mm). Avoid
Tenpower 60XG (Mooch flags low cycle life). Linkdata 65P, BAK 65E, Vapcell F66 top the 5 to 20 A columns but are not rated
for 30 A. On 2026-11-06 RS60 was out of stock at NKON and not listed at IMR Batteries; no trusted US vendor found for 60P or
60Q. Buy one of these from a reputable vendor if possible; otherwise P50B. Tell the power board thread which cell is bought.

**13S2P P50B:** 10.0 Ah, 46.8 V nominal, 468 Wh. That is 120 A continuous at the cells, so the 60 A fuse is the limit by a
factor of two (datasheet × 2). Charging stops at 4.15 V, so usable energy is about 450 Wh (estimate).

**Pack resistance (estimate):** 12.8 mΩ ÷ 2 = 6.4 mΩ per pair, × 13 = 83 mΩ, plus about 5 mΩ of plates, fuse, leads and
XT90 = **about 90 mΩ**. At 60 A the pack sags about 5.4 V and makes about 320 W of heat for as long as it lasts. At the
200 W bench average (4.3 A) that heat is 1.7 W, which doesn't matter. At 20 A continuous it is 36 W, so long hard runs will
warm the pack. That is what the two NTCs are for. The measured number replaces this estimate after section 8, step 6.

**Buying rules:**
- One order, one batch, all 28 cells together, so they share a date code. Matched cells make passive balancing enough.
- Buy only from a cell vendor that sells genuine Molicel: 18650BatteryStore, IMR Batteries, Liion Wholesale, Battery
  Junction (verify current stock at each). Never Amazon, eBay or AliExpress listings: fakes in 21700 are common.
- On arrival, check the laser-etched code on every can, the mass (69 to 71 g), and the resting voltage (shipped at about
  3.4 to 3.6 V). Any cell under 3.0 V or more than 30 mV from the median goes back.
- Cells ship ground-only as UN3480 dangerous goods. Expect a hazmat fee and no air shipping.

## 3. BMS: the board AFE for the robot, a JK for the bench

### 3.1 Robot settings (agreed with the power board thread)

| Function | Setting | Where |
| --- | --- | --- |
| Normal end of charge | 4.15 V per cell. The supervisor stops at the first cell to reach it; the charger's CV is 53.9 V | supervisor + charger |
| Cell over-voltage trip | 4.22 to 4.25 V, about 1 s delay (so braking into a full pack doesn't trip) | AFE |
| Cell under-voltage, soft | 3.0 V: the host warns, then disarms the motors | supervisor |
| Cell under-voltage, hard | 2.8 V trip (datasheet floor is 2.5 V) | AFE |
| Discharge over-current | 60 A for 1 s (AFE OCD3), plus a 64 A hardware comparator (board thread, main d9a7339) | AFE |
| Short circuit | about 150 A in 200 µs (board thread checks FET SOA) | AFE |
| Over-temperature | 60 °C discharge cutoff; charge allowed only 0 to 45 °C (P50B is rated −20 to 60 °C; we stay conservative) | AFE |
| Balancing | passive, about 150 mA, during charge above 3.9 V, cells more than 15 mV apart | AFE |
| Regen charging (Joey wants regen back into the cells, agreed 2026-11-06) | through the BMS FETs, counted as charge. Charge OC trip 48 A at 425 ms (12 mV on the 0.25 mΩ shunt; the BQ76952's longest delay; board thread, main d9a7339). P50B max charge is 25 A per cell. Host caps braking at 30 A from 0 to 95% charge, and at 5 A above 95% or below 5 °C. Clamp energy logged | AFE + host |
| Regen backstop | clamp at 57.5 V (4.42 V per cell) into the brake resistor | power board |

The board thread notes that 20 A of braking raises a full pack's terminal voltage to about 4.30 V per cell, which is why the
over-voltage trip has a delay. If it still trips, the host caps braking current above 95% charge.

### 3.2 Bench BMS (buy now)

A **JK 13S smart BMS** with Bluetooth: JK B2A20S15P class (verify the model, 8 to 20S, 2 A active balance, 150 A) or a JBD
SP15S 13S 60 A. About $50 to $90. Use it for:
- watching per-cell voltage during the first charge,
- the discharge and DCIR tests,
- the fallback if power board rev A has a problem.

It plugs into the pack through a **bench adapter harness**: Micro-Fit 2 × 10 to the JK's balance plug and NTC leads. So the
pack is built exactly the same way whether the board exists yet or not.

**Fallback-only rule (agreed):** the JK switches the low side. If it ever goes into the robot, nothing may reference pack B−
except the JK itself. Ask the board and trunk threads before doing it.

## 4. Busbars: plates, not strip

The pack doc listed 0.2 × 10 mm nickel for the series links. Rough sizing (estimate): pure nickel is about 7 × 10⁻⁸ Ωm. A
0.2 × 10 mm strip (2 mm²) over one 22.6 mm cell pitch is 0.8 mΩ. At 60 A that is about 3 W in one small strip. Forum rules of
thumb put that strip at 10 to 15 A continuous. That is too small.

The 4 × 7 grid (pack doc section 3) lets every series link be a **plate** covering four cells: two of one pair and two of the
next. A 1 × 4 plate (in-row link) has about 20 mm of width for the current to cross. A 2 × 2 plate (row-to-row link) has
about 43 mm.

| Plate material | In-row link at 60 A | Notes |
| --- | --- | --- |
| **0.15 mm pure nickel plate (cut from 40 mm+ wide strip or sheet)** | **0.53 mΩ, 1.9 W** | **first pack (Joey's call: a ~$40 handheld welder, which handles 0.15 mm but not 0.2 mm)** |
| 0.2 mm pure nickel plate | 0.40 mΩ, 1.4 W | needs a stronger welder |
| 0.15 mm nickel-plated copper, or a 0.1 mm Cu + 0.15 mm Ni sandwich | about 0.1 mΩ, 0.4 W | upgrade for the leg-rated pack. Needs a strong welder and practice |

Across 13 links the 0.15 mm nickel plates add about 5 mΩ (estimate) and about 19 W at a 60 A peak, for seconds. At 20 A
it is about 2 W. That's fine for arms and waist. A leg-rated pack needs copper busbars and a better welder.

- End plates (B0−, B13+): solder the 10 AWG lead to a **nickel tab that extends past the cell**, never to a cell. Pre-tin the
  tab before welding it on, so the soldering heat stays off the can.
- Taps: weld a small 0.15 mm nickel tab to each plate, then solder the 22 AWG tap wire to the tab, away from the can.
- Spot-weld the cells. Never solder them.

## 5. Charging

- **Voltage:** 53.9 V CV (4.15 V per cell), set and measured with a meter before the first use.
- **Current:** 4 A. That is 0.4 C. A full charge takes about 2.5 to 3 hours, and it stays inside the P50B's 5 A standard
  charge current with margin for the robot's compute load running at the same time.
- **Connector:** XT60 into the board's charge port (rated 5 A continuous, reverse-polarity protected, presence detected by
  voltage; a charger present blocks motor arming). Agreed with the board thread.

**Charger options** (verify each before buying; prices move):

| Charger | Why | Cost (verify) |
| --- | --- | --- |
| **Grin Satiator** (or similar programmable e-bike charger) | Set the voltage and current exactly, made for custom packs, reliable termination | about $300 to 350 |
| Adjustable 13S Li-ion charger, 54.6 V nominal, with an internal voltage trimmer | Cheap; trim to 53.9 V, verify on a meter, seal the trimmer | about $40 to 70 |
| Riden RD6024 lab supply (60 V, 24 A) + a 65 to 70 V input supply | Has a battery-charge mode. **Also a 48 V bench supply** for testing the arms at rated voltage without the pack. But it can't absorb regen, so it needs the brake resistor clamp in line | about $200 to 280 total |

**My recommendation:** the cheap trimmable charger to start, because the board's supervisor and the AFE both stop the charge
independently. Add the Riden later if the arm work wants a 48 V bench supply.

**Before the board exists:** charge through the JK at 2 A on the bench, attended. The JK's charge FET and per-cell display
are the protection.

## 6. Tools and consumables

| Item | Why | Cost (verify) |
| --- | --- | --- |
| **Spot welder: handheld battery type (Joey's pick)** | handles 0.15 mm pure nickel; pull-test scrap on its highest setting before touching cells | $40 |
| Cell IR meter, YR1035+ class (4-wire, AC 1 kHz), **optional** | grade every cell; a normal multimeter can't read 6 mΩ | $30 to 50 |
| Electronic load, 150 to 200 W, up to 150 V (DL24P class) | capacity test at C/3 and DCIR steps | $40 to 80 |
| Good multimeter (if not already owned) | voltage checks at every step | owned? |
| Steel tray or a cement board to charge on | | $20 |
| Insulated tools (or wrap the ends of pliers and cutters in Kapton), ceramic tweezers | a dropped tool across a plate is a dead short | $20 |
| Safety glasses, nitrile gloves | welding spatter | owned? |
| Nickel sheet/strip 0.2 × 40 mm+ (pure, test it: pure nickel sparks dull orange on a grinder and stays bright in salt water; plated steel rusts) | plates | $25 |
| 0.15 × 8 mm pure nickel | tap tabs | $10 |
| 21700 fish-paper rings (positive end, every cell), fish-paper sheet, Kapton tape | insulation | $15 |
| 21700 snap-together cell holders, 1 mm spacing | sets the grid | $12 |
| Silicone wire: 10 AWG red/black, 22 AWG in several colours | leads, taps | $20 |
| PVC heat-shrink wrap, about 200 mm flat width (or a printed lid only) | outer skin under the case | $10 |
| EVA foam, 2 mm | cushion between pack and case | $5 |
| Micro-Fit 3.0 housings + **pre-crimped leads** (Molex 2145xx series, verify part), JST-XH balance plug | sense harness and JK bench adapter, no crimper needed | $25 |

No crimper: Molex sells Micro-Fit contacts pre-crimped on wire (about $1 each, verify). Joey already owns a multimeter.

## 7. Safety rules

1. **The pack fuse and the E-stop are the safety system, not the BMS (decided).** The fuse sits on the pack as close as
   possible to B13+.
2. **Charge attended, on concrete or a steel tray.** Never overnight, never unattended in the robot
   until we have logged at least 10 clean charges with the board's supervisor.
3. **Build on a clear, non-flammable bench.** Only the cells being worked on are out. Everything else stays in its box,
   in groups of no more than a few cells. Watches and rings off.
4. **Never weld or solder with the series links complete unless needed.** Weld the parallel pairs first, then the series
   plates one at a time. The pack voltage climbs as you go; at 54 V a dropped tool across the stack is a real arc.
5. **Pack out of the robot:** XT90 capped (the female housing on the board side is shrouded; put a cap on the male pack
   side), sense connector in its holder, stored at 50 to 60% (about 3.75 V per cell) if it sits for more than two weeks.
6. **A swollen, hot, hissing or sweet-smelling pack goes outside onto concrete** and gets left alone. No water on a
   lithium fire indoors; get it outside if safe to, otherwise distance.
7. **Tip-over:** the frame (2020 posts on both sides, gussets outboard) and the shell's crush pad protect the pack. Check
   the case for cracks after any fall. A dented cell is scrap.
8. Store the 2 spare cells in a plastic cell case, never loose.

## 8. Build and test, step by step

Each step has a pass/fail check. A failure stops the build until it is understood.

| Step | Do | Pass criteria |
| --- | --- | --- |
| 1. Receive | Check codes, mass, voltage of all 28 cells | 69 to 71 g; all within 30 mV of the median; none under 3.0 V |
| 2. Grade | With an IR meter: AC IR of each cell. Without one: resting voltage after 24 h, plus the single-batch purchase. Log in a spreadsheet next to this doc | IR within ±10% of the median (about 6.5 mΩ AC datasheet), or voltage within 10 mV; reject outliers |
| 3. Match | Sort into 13 pairs so that pair IRs (parallel) are as even as possible; the 2 spares are the outliers | pair IRs within ±5% |
| 4. Dry fit | Holders, fish-paper rings on every positive end, polarity marked with a paint pen on the holder, series path drawn on the holder face | photo, matches the layout drawing |
| 5. Weld | Pull-test scrap first (the weld should tear the nickel, not peel). Parallel pairs first, then series plates. 4 welds per cell end | every weld passes a gentle pry with a plastic spudger; each pair reads the same voltage |
| 6. Sense and measure | Weld tap tabs, wire the sense board, connect the JK through the bench harness. **Measure pack DCIR**: ΔV between a 1 A and a 5 A load step, 10 s each | all 13 cell voltages within 10 mV; DCIR 70 to 110 mΩ. **Send the number to the board thread** |
| 7. Close | Fuse, XT90S, NTCs (one in the middle of the stack, one at a stack edge, one next to the fuse), foam, wrap, printed case | insulation check: no continuity from any plate to the case's outside |
| 8. First charge | JK at 2 A to 53.9 V, attended, logging every 15 min | cells stay within 20 mV; pack under 35 °C |
| 9. Capacity | Electronic load at 3 A (C/3) to 3.0 V per cell | at least 9.4 Ah (94% of 10 Ah); cells within 50 mV at the end |
| 10. Recharge and rest | Charge to 53.9 V, rest 7 days, measure | self-discharge under 20 mV per cell over 7 days |
| 11. Onto the robot | Only through the power board (or the JK fallback) with E-stop and pack fuse in place. Arms on first; legs need the 3P or second-pack decision | board reads the same cell voltages as the JK within 10 mV |

Record the results (cell IRs, pair sorting, DCIR, capacity, charge logs) in `hardware/electrical/power/pack-test-log.md`
when the build happens. Per the review gates, a review packet with these results goes out **before the pack is first
connected to the robot** (gate 3: load-bearing or powered).

## 9. Shopping list and cost

Pack-only budget (Joey, 2026-11-06): the **BMS and charger count against the power board**, Joey owns a meter, uses a ~$40
handheld welder, needs no crimper (pre-crimped leads), and skips the extinguisher.

| Item | Cost (verify) | Note |
| --- | --- | --- |
| Molicel P50B × 28 | $294 | vendor page, $10.50 at 10+. P45B at about $6, if back in stock, saves about $120 |
| Nickel plate 0.15 mm (40 mm+ wide) + 0.15 × 8 mm tab strip | $30 | |
| Cell holders, fish paper, Kapton, foam, wrap | $45 | |
| 60 A pack fuse **rated ≥ 58 V DC (never a 32 V automotive part)** + matching holder, spare fuse | $15 to 40 | see section 1 fuse note |
| XT90S, XT60, 10 AWG + 22 AWG silicone wire | $35 | |
| Micro-Fit 2 × 10 housings + pre-crimped leads, 3 NTCs, sense board (in the power board's PCB order) | $30 | |
| Printed case (PC FR or ASA) | $15 | filament |
| Handheld spot welder | $40 | Joey's pick |
| **Total, pack** | **about $505** | |
| *On the power board budget:* JK 13S bench BMS $60, trimmable 4 A charger $50 | *$110* | |

Optional later: a 150 W electronic load ($40) for a controlled capacity test (otherwise the board's coulomb counter measures
capacity); a stronger welder for a copper-busbar leg pack; a cell IR meter ($35) if the multimeter can't grade cells (a DC
meter can't read 6 mΩ, so grading falls back to matching resting voltage plus the vendor's batch).

## 10. Pack sense board (new part, handed to the power board thread)

A small PCB (about 40 × 25 mm, estimate) sitting in the two spare grid positions at the top corner, next to the fuse.

- 14 tap inputs (B0 to B13) on solder pads, each through a **0.5 to 1 A fast fuse** (0603/0805 SMD, rated at least 63 V DC,
  verify) or a fusible PCB trace. The AFE's own input resistors stay on the main board.
- 3 NTC inputs, plus the 2-pin connector-present loop.
- One **Molex Micro-Fit 3.0, 2 × 10 right-angle header** (agreed with the board thread, which builds the board in its PCB order).
- Conformal coat, Kapton over the back.
- Tap order on the 2 × 10 header is a **zigzag**: adjacent pins across a column are one cell apart (4.2 V), and pins along
  a row are two cells apart (8.4 V). Never put B0 next to B13. A bent pin then shorts at most 8.4 V, and always through a tap fuse.

The power board thread designs it into the same PCB order; this thread owns its placement in the pack.

## 11. Open items

| Item | Owner | Status |
| --- | --- | --- |
| BQ76952 open-wire detection timing (section 12.11) | power board thread | checking; connector settled at 2 × 10 |
| Measured pack DCIR | this thread | after build step 6 |
| Measured per-arm bus current | arm trunk thread | after the bench shoulder runs |
| Legs: 13S3P (39 cells, about 3.3 kg) or a pelvis pack in parallel | this thread + frame | later, when the leg loads are known |
| D2 door grown to pass a 95 × 162 pack | body shell thread | already listed |
| Pack case CAD (printed, rails, pull tab, lid facing the board at Z 34 with 0.5 mm Nomex) | this thread, on Joey-PC | next, after cells arrive and the plate layout is drawn |
