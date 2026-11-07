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
- **BMS: an off-the-shelf JK smart BMS on the pack, JK-BD6A17S8P (Joey's decision, 2026-11-06 22:44Z).** It is 80 A
  continuous / 150 A peak, common port, 0.6 A active balance, Bluetooth, RS485/CAN through add-on modules, and
  133 × 81 × 18 mm, so it fits the 170 × 90 × 20 mm rear-face envelope. It switches the low side, so **nothing may touch
  pack B− except the JK**. It has no precharge and no hardware remote-off input (section 3). The pack sense board and the
  2 × 10 connector are dropped. The pack is cells, Class T fuse, JK, AS150U, and a charge XT60.
- **Charge to 4.15 V per cell (53.9 V), not 4.2 (agreed).** This leaves room for regen in a full pack and adds cycle life
  for about 3% of capacity. Use an adjustable-voltage charger at 4 A, ending in an XT60.
- **Busbars are wide plates, not 10 mm strip.** A 10 mm nickel strip cannot carry 60 A. The pack doc's nickel line is
  replaced (section 4).
- **Tools to buy: a ~$40 handheld spot welder.** Joey has the meter; pre-crimped leads replace a crimper.
- **Pack total about $480 including the welder** (section 9). The JK BMS and charger (about $145) count against the power board.

## 1. What the pack is, and what it is not

```
 cells 13S2P (26 x P50B, wide Ni plates)
   B13+ ── 60 A Class T fuse ── 8 AWG ──────────────────────── P+ ──┬── AS150U ─► power board (rear top corner, D2)
   B0−  ── JK B− │JK-BD6A17S8P│ C−/P− ── 8 AWG ──────────────── P− ──┤
   B0..B13 taps ─┘ (JK balance plug)  2 NTC on the cells         └── XT60 charge pigtail (charger, 53.9 V CC/CV)
```

- **On the pack:** cells, plates, Class T fuse, the JK BMS on the rear face, AS150U, XT60 charge pigtail, the JK's two NTC
  probes, printed case, foam, wrap.
- **Ground rule (low-side BMS):** the robot's only ground is P− (the JK's C−). Nothing touches B−: not the charger, not
  the regen clamp, not the bench supply, not a laptop. Charger, board and robot all connect at P+/P−.
- **Consequence:** the pack protects itself out of the robot and can charge anywhere (on the bench or in the robot).

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

### History

Until 2026-11-06 22:44Z the BMS was planned on the power board (BQ76952, high-side) with a fused sense board and a 2 × 10
Micro-Fit to the pack. Joey's minimal-board restart (pdb/REV_B_CLEAN_SHEET.md, REV_B_PARTS.md) moved it to an
off-the-shelf JK on the pack. Git history keeps the old design.

### Where the pack mates (board thread, main 9ce67a0)

The power connector mates at the **pack's rear top corner, behind the D2 door**. The board's soldered 8 AWG pair runs
over the pack's top edge (inside the strap window, X −5 to +25) to an Amass AS150U female (rev B5, Joey: the pack-to-board run
is the heaviest in the robot). The pack side has a short AS150U male pigtail (anti-spark, about 60 mm, 8 AWG) from P+/P−, parked in a printed saddle so nothing hangs loose when the door is open.

### Pack heat for the shell's airflow sizing (estimate, for frame gate review 2 item F)

Pack heat is I² × R. R is about 90 mΩ for P50B, plus or minus 20%; the 6 Ah cells are unmeasured. The pack's thermal mass is
about 2 kJ/K (26 cells × 70 g × about 1 J/g·K, plus case).

| Case | Current | Heat | Effect |
| --- | --- | --- | --- |
| Bench average, arms + compute | 4 to 5 A | about 2 W | negligible |
| Hard arm session, sustained | 15 A | about 20 W | about +10 K over 30 min with natural convection (estimate) |
| Worst sustained we'd allow | 20 A | about 36 W | about +15 to 20 K over 30 min; NTC-driven fan in the lower box |
| 40 A burst (reviewer case), 10 s | 40 A | 144 W | about 1.4 kJ, under +1 K. Not a steady state |
| 60 A peak (fuse/OCD), 1 s | 60 A | 324 W | negligible energy |

**Size the lower-box airflow for about 20 W steady, with 36 W as the worst case.** 40 A is a burst current, not a duty
cycle. The AFE's 60 °C discharge cutoff and the NTC-driven fans are the backstop. Measured numbers replace this table after
build step 6 and the first logged sessions.

### Retention bosses for the frame cradle (Joey's cradle decision, 2026-11-06 16:32Z)

The cradle is printed, with a bottom lip, a front lip, a top strap, and **no rear bar**. A side thumbscrew on each side
goes through the tray rail into the case. The rear face stays free for a possible on-pack BMS.

- **One M4 heat-set insert per side face (|X| 47.5) at Y 252.6 ± 0.5** (alternative Y 275.3), anywhere in Z −25 to +15.
  This is the cusp between two cells of the outer column: row centres sit at Y 195.9 + k × 22.7. The 2 mm wall thickens
  inward into the cusp to about 6 mm, which is the only place a 5.6 mm insert fits clear of the cells. The Z range keeps
  it clear of the holder plates.
- **The thumbscrew must bottom on a shoulder or be length-limited,** so its tip never reaches more than 6 mm in from the
  outer side face. Any further reaches a cell can.
- Keep tap wires and NTC leads out of those two cusps.
- **Top-edge lead window (frame cradle, main bc03e22):** the top strap has a 30 mm gap at X −5 to +25 with 8.6 mm of height
  (Y 345 to 353.6). The 10 AWG power pair crosses the pack top only inside that window. The sense lead may run under the strap
  hooks. Keep the case top flat there, with no bosses or labels proud of Y 345.

### Rear-corner layout (provisional, for the shell's D2 check; limits |X| ≤ 45, Z ≥ −68)

| Part | Position (torso coords) | Note |
| --- | --- | --- |
| Pack fuse | inside the case, empty top-corner grid slot, about X +34, Y 333, Z −40 to +30, axis along Z | nothing behind the rear face |
| Mated AS150U pair | flat on the rear face, axis along X: X −15 to +43, Y 323 to 345, Z −45 to −60 incl. a printed saddle | pack pigtail enters at the +X end; board pair comes over the top in X −5 to 25 and enters at the −X end |
| JK-BD6A17S8P BMS | rear face, long axis along Y: X −40.5 to +40.5, Y 186 to 319, Z −45 to −65 (18 mm board + about 2 mm standoffs) | FET heatsink faces rearward toward D2 |
| XT60 charge pigtail | parked flat at X −42 to −17, Y 325 to 345, Z −45 to −58 | reached through D2 |

AS150U: 70 A continuous, 140 A instantaneous, 8 AWG (retailer listings; no Amass datasheet found, confirm with Amass). Its envelope is larger than the XT90's that this table was drawn for: re-check the saddle and the mated length against the part before printing.

### Connector rule

Only one power connector now: the AS150U (anti-spark) mates last and comes off first. The JK's balance plug stays inside the
pack and is never unplugged in service. When building, plug the JK's balance harness in **only after checking every tap
voltage in order with the meter**. Then connect B−, then the NTCs. Follow the JK manual's order (verify).

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
AS150U = **about 90 mΩ**. At 60 A the pack sags about 5.4 V and makes about 320 W of heat for as long as it lasts. At the
200 W bench average (4.3 A) that heat is 1.7 W, which doesn't matter. At 20 A continuous it is 36 W, so long hard runs will
warm the pack. That is what the two NTCs are for. The measured number replaces this estimate after section 8, step 6.

**Buying rules:**
- One order, one batch, all 28 cells together, so they share a date code. Matched cells make passive balancing enough.
- Buy only from a cell vendor that sells genuine Molicel: 18650BatteryStore, IMR Batteries, Liion Wholesale, Battery
  Junction (verify current stock at each). Never Amazon, eBay or AliExpress listings: fakes in 21700 are common.
- On arrival, check the laser-etched code on every can, the mass (69 to 71 g), and the resting voltage (shipped at about
  3.4 to 3.6 V). Any cell under 3.0 V or more than 30 mV from the median goes back.
- Cells ship ground-only as UN3480 dangerous goods. Expect a hazmat fee and no air shipping.

## 3. BMS: JK-BD6A17S8P on the pack

### 3.1 Model choice

| Model | Rating | Size | Comms | Fits 170 × 90 × 20? |
| --- | --- | --- | --- | --- |
| **JK-BD6A17S8P** | 80 A cont / 150 A peak, 8 to 17S, 0.6 A active balance, common port, 3 temperature sensors | **133 × 81 × 18 mm** | Bluetooth; RS485 or CAN via optional module | **yes** |
| JK-BD6A17S6P / BD6A20S6P | 60 A / 100 A, charge OC 10 to 60 A | 115 × 78 × 17.6 mm | Bluetooth; comms port shared with BT module (verify RS485) | yes, but no headroom over the 60 A fuse |
| JK-BD6A20S10P | 100 A / 200 A | 162 × 102 × 20.4 mm | RS485, CAN via P5 | no: 12 mm too wide |
| JK-B2A24S15P and up | 150 to 200 A, 2 A balance | 162 × 102 × 20.4 mm | RS485/CAN | no |

Sources (vendor and manual, verify on the unit): [evlithium JK-BD6A17S8P](https://evlithium.com/Battery-Management-System/jk-bd6a17s8p.html),
[18650BatteryStore listing](https://www.18650batterystore.com/products/jk-bd6a17s8p-bms) ($85, sold out 2026-11-06),
[NKON listing](https://nkon.nl/en/jk-bms-bd6a17s8p.html), [JK active-balance BMS manual](https://cdn.shopify.com/s/files/1/0007/0824/7615/files/JK-Smart-BMS-with-Active-Balancer-Instruction-EN.pdf).
**Order the RS485 (or CAN) add-on module with it**, because the Pi needs the telemetry. A USB-RS485 adapter is about $10.

**What it doesn't have (manual, forum check):**
- **No precharge.** The power board keeps every bulk capacitor behind its soft-start E-stop bank, so the JK's output only
  sees a soft load (agreed). Nothing with large input capacitance may connect straight to P+/P−.
- **No hardware remote-off input.** The manual says the board has no power-on control switch; charge and discharge are
  switched from the app or over the comms port. So **the JK cannot be the E-stop's second break.** A software command over
  RS485 is fine as a "soft off", but not as a safety function. The board's hardware E-stop bank is the safety path.
- **Short-circuit protection** is built in. The trip delay is not stated (verify on the unit). Recovery defaults to 60 s.
- Wakes when a charger is connected that is at least 5 V above the pack.

### 3.2 Settings (program over Bluetooth, then record the screen in pack-test-log.md)

| Function | Setting | Why |
| --- | --- | --- |
| Cell count / chemistry | 13S, Li-ion (NCM) | |
| Normal end of charge | charger CV 53.9 V (4.15 V per cell) | regen headroom, cycle life |
| Cell over-voltage protection | 4.22 V, release 4.10 V, delay as long as the app allows (about 1 s+) | braking into a full pack shouldn't trip it |
| Cell under-voltage protection | 2.8 V, release 3.0 V | host disarms motors at 3.0 V from telemetry |
| Discharge over-current | 80 A, longest delay | above the limb fuses, so a limb fault blows its own fuse first and the Pi stays up |
| Charge over-current (regen + charger) | 50 A, about 1 s delay | P50B max charge is 25 A per cell. Host caps braking at 30 A (5 A above 95% charge or below 5 °C) |
| Short circuit | on (default) | |
| Charge temperature | 0 to 45 °C | |
| Discharge temperature | up to 60 °C | |
| Balancing | start 3.9 V, trigger 10 mV | 0.6 A active |
| Regen backstop | power board clamp at 57.5 V (4.42 V per cell) into the brake resistor | when the JK opens its charge FET or the pack is full |

If the JK opens its charge FET, discharge still flows through the FET body diode (common-port BMS behaviour, verify), so the
robot stays up and regen goes to the clamp.

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
- **B0 and B13 taps are their own tabs (Kelvin sense)**, welded to the end plates apart from the 10 AWG power tabs, so no
  pack current flows in a sense wire (board gate-3 review, main 3c1dab91). These two taps go to the JK's B0/B13 balance pins.
- Spot-weld the cells. Never solder them.

## 5. Charging

- **Voltage:** 53.9 V CV (4.15 V per cell), set and measured with a meter before the first use.
- **Current:** 4 A. That is 0.4 C. A full charge takes about 2.5 to 3 hours, and it stays inside the P50B's 5 A standard
  charge current with margin for the robot's compute load running at the same time.
- **Connector:** the pack's own XT60 charge pigtail on P+/P− (through the JK, common port). It works in or out of the robot.
  The host blocks motor arming while JK telemetry shows charge current.

**Charger options** (verify each before buying; prices move):

| Charger | Why | Cost (verify) |
| --- | --- | --- |
| **Grin Satiator** (or similar programmable e-bike charger) | Set the voltage and current exactly, made for custom packs, reliable termination | about $300 to 350 |
| Adjustable 13S Li-ion charger, 54.6 V nominal, with an internal voltage trimmer | Cheap; trim to 53.9 V, verify on a meter, seal the trimmer | about $40 to 70 |
| Riden RD6024 lab supply (60 V, 24 A) + a 65 to 70 V input supply | Has a battery-charge mode. **Also a 48 V bench supply** for testing the arms at rated voltage without the pack. But it can't absorb regen, so it needs the brake resistor clamp in line | about $200 to 280 total |

**My recommendation:** the cheap trimmable charger to start. The charger's CV stops the charge and the JK's 4.22 V cell
protection is the backstop. Add the Riden later if the arm work wants a 48 V bench supply.

**First charges:** at 2 A on the bench, attended, watching per-cell voltages in the JK app.

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
| XT60 pair for the charge pigtail, JK balance harness (comes with the BMS) | charge lead | $5 |

No crimper needed. Joey already owns a multimeter.

## 7. Safety rules

1. **The pack fuse and the E-stop are the safety system, not the BMS (decided).** The fuse sits on the pack as close as
   possible to B13+.
2. **Charge attended, on concrete or a steel tray.** Never overnight, never unattended in the robot
   until we have logged at least 10 clean charges with the board's supervisor.
3. **Build on a clear, non-flammable bench.** Only the cells being worked on are out. Everything else stays in its box,
   in groups of no more than a few cells. Watches and rings off.
4. **Never weld or solder with the series links complete unless needed.** Weld the parallel pairs first, then the series
   plates one at a time. The pack voltage climbs as you go; at 54 V a dropped tool across the stack is a real arc.
5. **Pack out of the robot:** AS150U capped (the female housing on the board side is shrouded; put a cap on the male pack
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
| 6. BMS and measure | Weld tap tabs, check every tap voltage in order, then plug in the JK balance harness. **Measure pack DCIR**: ΔV between a 1 A and a 5 A load step, 10 s each | all 13 cell voltages within 10 mV in the JK app; DCIR 70 to 110 mΩ. **Send the number to the board thread** |
| 7. Close | Fuse, XT90S, NTCs (one in the middle of the stack, one at a stack edge, one next to the fuse), foam, wrap, printed case | insulation check: no continuity from any plate to the case's outside |
| 8. First charge | JK at 2 A to 53.9 V, attended, logging every 15 min | cells stay within 20 mV; pack under 35 °C |
| 9. Capacity | Electronic load at 3 A (C/3) to 3.0 V per cell | at least 9.4 Ah (94% of 10 Ah); cells within 50 mV at the end |
| 10. Recharge and rest | Charge to 53.9 V, rest 7 days, measure | self-discharge under 20 mV per cell over 7 days |
| 11. Onto the robot | Through the power board's soft-start E-stop bank, pack fuse in place, nothing on B−. Arms on first; legs need the 3P or second-pack decision | JK telemetry reaches the Pi over RS485/CAN |

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
| AS150U pair, XT60, 8 AWG + 22 AWG silicone wire | $40 | |
| XT60 charge pigtail parts | $5 | |
| Printed case (PC FR or ASA) | $15 | filament |
| Handheld spot welder | $40 | Joey's pick |
| **Total, pack** | **about $480** | |
| *On the power board budget (Joey):* JK-BD6A17S8P + RS485/CAN module about $95, trimmable 4 A charger $50 | *about $145* | |

Optional later: a 150 W electronic load ($40) for a controlled capacity test (otherwise the board's coulomb counter measures
capacity); a stronger welder for a copper-busbar leg pack; a cell IR meter ($35) if the multimeter can't grade cells (a DC
meter can't read 6 mΩ, so grading falls back to matching resting voltage plus the vendor's batch).

## 10. Pack sense board

Dropped 2026-11-06 22:44Z with the move to the on-pack JK. The JK's own balance harness connects straight to the taps.

## 11. Open items

| Item | Owner | Status |
| --- | --- | --- |
| JK-BD6A17S8P: buy with the RS485 or CAN module; verify size, short-circuit delay and body-diode behaviour on the unit | this thread | when the pack is bought |
| Measured pack DCIR | this thread | after build step 6 |
| Measured per-arm bus current | arm trunk thread | after the bench shoulder runs |
| Legs: 13S3P (39 cells, about 3.3 kg) or a pelvis pack in parallel | this thread + frame | later, when the leg loads are known |
| D2 door grown to pass a 95 × 162 pack | body shell thread | already listed |
| Pack case CAD (printed, rails, pull tab, lid facing the board at Z 34 with 0.5 mm Nomex) | this thread, on Joey-PC | next, after cells arrive and the plate layout is drawn |
