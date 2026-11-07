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
- **BMS: JBD SP17S005, 60 A, 13S Li-ion, common port, on the pack; Daly smart 13S 60 A as backup (rev B5, 2026-11-07;
  the JK-BD6A17S8P at about $100 was dropped on cost).** Bluetooth and UART telemetry, 102 × 65 × 10 mm, short circuit
  900 ± 180 A at 400–800 µs. It switches the low side, so **nothing may touch pack B− except the
  BMS**. It has no precharge and no hardware remote-off input (section 3). The pack sense board and the 2 × 10 connector are
  dropped. The pack is cells, Class T fuse, BMS, XT90S, and a charge XT60.
- **Charge to 4.15 V per cell (53.9 V), not 4.2 (agreed).** This leaves room for regen in a full pack and adds cycle life
  for about 3% of capacity. Use an adjustable-voltage charger at 4 A, ending in an XT60.
- **Busbars are wide plates, not 10 mm strip.** A 10 mm nickel strip cannot carry 60 A. The pack doc's nickel line is
  replaced (section 4).
- **Tools to buy: a ~$40 handheld spot welder.** Joey has the meter; pre-crimped leads replace a crimper.
- **Pack total about $480 including the welder** (section 9). The BMS and charger (about $100) count against the power board.

## 1. What the pack is, and what it is not

```
 cells 13S2P (26 x P50B, wide Ni plates)
   B13+ ── 60 A Class T fuse ── 8 AWG ──────────────────────── P+ ──┬── XT90S ──► power board (rear top corner, D2)
   B0−  ── BMS B−│ JBD 13S 60A│ C−/P− ── 8 AWG ──────────────── P− ──┤
   B0..B13 taps ─┘ (BMS bal. plug)   2 NTC on the cells         └── XT60 charge pigtail (charger, 53.9 V CC/CV)
```

- **On the pack:** cells, plates, Class T fuse, the BMS on the rear face, XT90S, XT60 charge pigtail, the BMS's NTC
  probes, printed case, foam, wrap.
- **Ground rule (low-side BMS):** the robot's only ground is P− (the BMS's C−/P−). Nothing touches B−: not the charger, not
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
off-the-shelf smart BMS on the pack (JK at first, JBD from rev B5). Git history keeps the old design.

### Where the pack mates (board thread, main 9ce67a0)

The power connector mates at the **pack's rear top corner, behind the D2 door**. The board's soldered 8 AWG pair runs
over the pack's top edge (inside the strap window, X −5 to +25) to an XT90 (rev B5: 8 AWG fine-strand silicone, the heaviest run in
the robot; the XT90 solder cups take 8 AWG). The pack side has the XT90S (anti-spark) half mounted in the case wall at the rear top corner (rear-corner table), with its leads inside the case.

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
| Pack-side XT90S half, **case-wall mount (option a, 2026-11-07)** | captured in a printed pocket at the rear top corner, X +10 to +45, Y 323 to 345, Z −45 to −60, mating face toward −X; leads stay inside the case (short 8 AWG runs to the fuse and BMS P−), no external bend | the board half plugs in from −X; its 8 AWG bend room falls between the XT90S and the XT60. The pocket clamps the housing with a printed collar and two M3 screws so mating force (about 40 N, verify) and pull-out never load the solder joints |
| BMS (JBD SP17S005, 102 × 65 × 10 mm) | rear face, long axis along Y, inside X −40.5 to +40.5, Y 186 to 319, Z −45 to −65 (the old JK envelope; the JBD uses about 102 of the 133 mm) | FET side faces rearward toward D2 |
| XT60 charge pigtail | **parked flat on the pack TOP (2026-11-07, frame thread)**: X −45 to −20, Z −45 to −20, lying along Z, in the 8.6 mm band between the pack top (Y 345) and the shared rail (Y 353.6); XT60 is about 8.2 mm tall (verify). Clear of the strap (Z −15.5 to 5.5) and the 8 AWG path (X −5 to 25) | reached over the pack through D2. Use 14 AWG leads and a recessed printed clip so nothing stands proud of Y 353 when the pack slides |

XT90 sizes are from memory (verify): about 22 × 12 mm section, about 45 mm mated length. Rating: Amass XT90 45 A continuous, enough for arms and waist. The AS150U (70–80 A) was modelled by the frame thread on 2026-11-07 and doesn't fit this corner: 8 AWG bend room needs about 100 mm in X where the corner has 90. **Open for legs (rev C): a pack-corner redesign, e.g. a recessed vertical AS150 pocket.**

8 AWG harness, board side: the pair runs up the board's component face from its pads and bends over the board's top edge at R15 (bend starting near Y 330), flat, no sleeve, **7.0 mm max OD at the carrier edge**.

### Connector rule

Only one power connector now: the XT90S (anti-spark) mates last and comes off first. The BMS's balance plug stays inside the
pack and is never unplugged in service. When building, plug the BMS's balance harness in **only after checking every tap
voltage in order with the meter**. Then connect B−, then the NTCs. Follow the BMS manual's order (verify).

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

## 3. BMS: JBD SP17S005, 13S Li-ion, 60 A, common port (Daly backup)

Rev B5 (Joey, 2026-11-07): not paying about $100 for the JK. The board does the safety work (soft-start banks, port
breakers, clamp, E-stop), so the BMS only has to protect the cells, report telemetry and open on a hard short.

### 3.1 The pick

**JBD (Jiabaida) SP17S005, 60 A version, set to 13S, ternary Li-ion (NMC), not LiFePO4.** From the JBD spec sheet
(jiabaida.com SP17S005, [copy on AliExpress's CDN](https://ae01.alicdn.com/kf/Sb06fca3ee8ac4e26a9c68014176cd670D.pdf)):

| Item | SP17S005, 60 A version |
| --- | --- |
| Strings, chemistry | 10–17S, picks the count automatically (set 13 in the app if it misreads); order the **Li-ion** version |
| Port | common port (charge and discharge share P−), low-side MOSFETs, 80 V TOLL parts, ≤ 10 mΩ path |
| Comms | Bluetooth (built in) and non-isolated UART (standard); no RS485 or CAN on this model. The Pi reads it over UART with a USB-UART lead |
| Size | **102 × 65 × 10 mm** (80 A version 12 mm thick): fits the 170 × 90 × 20 mm rear-face space easily |
| Charge over-current | 70 ± 5 A, 7–13 s |
| Discharge over-current 1 | 70 ± 5 A, 20–40 s |
| Discharge over-current 2 | 225 ± 45 A, 200–800 ms |
| Short circuit | **900 ± 180 A, 400–800 µs**, recovers 5 s after the load is removed |
| Cell over-voltage range | up to 4.25 V (4.20 V default; set 4.22 V) |
| Balance | passive, 30–90 mA, from 3.90 V |
| Draw | ≤ 10 mA running, sleeps after 60 s with no current or comms |
| Price | about $40–65 depending on seller and add-ons |

**Where to buy (verify before ordering).** I could not confirm which online store is JBD's own:
- [jiabaida-bms.com, 13S 11–15S 20–60 A smart BMS](https://jiabaida-bms.com/products/jbd-smart-bms-13s-14s-11-15s-48v-20a-30a-40a-50a-60a-lithium-battery-pcb-with-balance-uart-rs485-1): the 13S Li-ion 60 A option is about $64–67 (the higher price includes RS485). The site carries the Jiabaida name; not verified as factory-owned.
- AliExpress store 1379203 is called "the JBD store" in an Endless-Sphere post (one buyer had a unit fail right away); not verified.
- On delivery, check that the silkscreen says SP17S005, that the FETs are 80 V TOLL parts, and that the app reads 13 cells at the meter's voltages.

**Endless-Sphere "JBD smart BMS heads up" thread.** The site's bot check blocked every fetch, so I only have search
snippets. The warning there is about the wiring: the 10–17S instructions are hard to follow (one buyer had to get a
diagram from the seller), and a wrong cell count is fixed in the app's parameters if the wiring is right. Others report
the app dropping its connection. **None of the snippets mention counterfeits or a firmware safety fault.** What this means
for us: wire the balance leads to JBD's 13S diagram, meter every tap before plugging in (build step 6), and confirm 13 cells
in the app. Read the thread in a browser before buying.

**Backup: Daly smart BMS, Li-ion 13S 48 V 60 A, common port, UART plus Bluetooth** ([Amazon B09Y5W5XPB](https://www.amazon.com/dp/B09Y5W5XPB),
DALY brand listing; about 128 × 66 × 13 mm). Short-circuit delay 200–500 µs by listing; the threshold is not published (a
non-smart sheet gives discharge OC 120 A). Many cheap Daly units on eBay and AliExpress are fakes, so buy from the DALY
brand store only. Price about $50–75.

### 3.2 What the board assumed from the JK, re-checked for the JBD

- **BMS close into the absorber** (absorber_sim close case, 8 cans): 345–395 A peak at 0.1 Ω loop, up to 590 A at 0.05 Ω,
  above 180 A for 36–52 µs. That is under the JBD's 720 A minimum short-circuit level, and the 225 A level needs 200 ms or
  more. **It cannot trip on the close.** For Daly, the 200 µs minimum delay is 4× the pulse. Bring-up step 2 still gates 10b.
- **Reverse polarity: the JBD is no backstop.** D1's crowbar current (500–1000 A) may sit under the JBD's 720–1080 A
  short-circuit level, which leaves the 200–800 ms over-current trip; D1 survives about 0.4–1.6 ms. The Class T is
  slower still. **The keyed XT90 and the bring-up polarity check carry this, as already agreed.**
- **No precharge, no hardware off switch** (the JBD has an optional soft switch, not a safety input). Unchanged: every bulk
  capacitor on the board sits behind the soft-start banks, the XT90S anti-spark resistor precharges the absorber cans at
  plug-in, and the board's hardware E-stop bank stays the safety path.
- **Peak current.** The board's 90 A peaks last far less than the 20–40 s level-1 delay. Above 65 A for 20 s it trips.
- **Charge and regen.** The JBD's charge over-current is 70 ± 5 A, above what the cells should take (P50B 25 A each), so
  the BMS doesn't protect the cells against regen. The host's 30 A braking cap and the board's 57.5 V clamp do. When the BMS
  opens its charge FET the robot still runs through the FET body diode (common-port behaviour, verify), and regen goes to
  the clamp.
- **Balancing is weak** (30–90 mA passive vs the JK's 0.6 A active). Fine for a new, matched pack. Watch the cell spread in
  the app; if it grows past 30 mV, balance on the bench.

### 3.3 Settings (program over Bluetooth, then record the screen in pack-test-log.md)

| Function | Setting | Why |
| --- | --- | --- |
| Cell count / chemistry | 13S, Li-ion (NMC) | |
| Normal end of charge | charger CV 53.9 V (4.15 V per cell) | regen headroom, cycle life |
| Cell over-voltage protection | 4.22 V, release 4.10 V, delay as long as the app allows (about 1 s+) | braking into a full pack shouldn't trip it |
| Cell under-voltage protection | 2.8 V, release 3.0 V | host disarms motors at 3.0 V from telemetry |
| Over-current, short circuit | leave the 60 A version's hardware table (70 A / 225 A / 900 A) | the board's port breakers and banks handle every board-side fault faster |
| Charge temperature | 0 to 45 °C | |
| Discharge temperature | up to 60 °C | |
| Balancing | start 3.9 V, trigger 15 mV (JBD defaults) | |
| Regen backstop | power board clamp at 57.5 V (4.42 V per cell) into the brake resistor | when the BMS opens its charge FET or the pack is full |

If the BMS opens its charge FET, discharge still flows through the FET body diode (common-port BMS behaviour, verify), so the
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
  pack current flows in a sense wire (board gate-3 review, main 3c1dab91). These two taps go to the BMS's B0/B13 balance pins.
- Spot-weld the cells. Never solder them.

## 5. Charging

- **Voltage:** 53.9 V CV (4.15 V per cell), set and measured with a meter before the first use.
- **Current:** 4 A. That is 0.4 C. A full charge takes about 2.5 to 3 hours, and it stays inside the P50B's 5 A standard
  charge current with margin for the robot's compute load running at the same time.
- **Connector:** the pack's own XT60 charge pigtail on P+/P− (through the BMS, common port). It works in or out of the robot.
  The host blocks motor arming while BMS telemetry shows charge current.

**Charger options** (verify each before buying; prices move):

| Charger | Why | Cost (verify) |
| --- | --- | --- |
| **Grin Satiator** (or similar programmable e-bike charger) | Set the voltage and current exactly, made for custom packs, reliable termination | about $300 to 350 |
| Adjustable 13S Li-ion charger, 54.6 V nominal, with an internal voltage trimmer | Cheap; trim to 53.9 V, verify on a meter, seal the trimmer | about $40 to 70 |
| Riden RD6024 lab supply (60 V, 24 A) + a 65 to 70 V input supply | Has a battery-charge mode. **Also a 48 V bench supply** for testing the arms at rated voltage without the pack. But it can't absorb regen, so it needs the brake resistor clamp in line | about $200 to 280 total |

**My recommendation:** the cheap trimmable charger to start. The charger's CV stops the charge and the BMS's 4.22 V cell
protection is the backstop. Add the Riden later if the arm work wants a 48 V bench supply.

**First charges:** at 2 A on the bench, attended, watching per-cell voltages in the BMS app.

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
| XT60 pair for the charge pigtail, BMS balance harness (comes with the BMS) | charge lead | $5 |

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
| 6. BMS and measure | Weld tap tabs, check every tap voltage in order, then plug in the BMS balance harness. **Measure pack DCIR**: ΔV between a 1 A and a 5 A load step, 10 s each | all 13 cell voltages within 10 mV in the BMS app; DCIR 70 to 110 mΩ. **Send the number to the board thread** |
| 7. Close | Fuse, XT90S, NTCs (one in the middle of the stack, one at a stack edge, one next to the fuse), foam, wrap, printed case | insulation check: no continuity from any plate to the case's outside |
| 8. First charge | Charger at 2 A to 53.9 V, attended, logging every 15 min | cells stay within 20 mV; pack under 35 °C |
| 9. Capacity | Electronic load at 3 A (C/3) to 3.0 V per cell | at least 9.4 Ah (94% of 10 Ah); cells within 50 mV at the end |
| 10. Recharge and rest | Charge to 53.9 V, rest 7 days, measure | self-discharge under 20 mV per cell over 7 days |
| 11. Onto the robot | Through the power board's soft-start E-stop bank, pack fuse in place, nothing on B−. Arms on first; legs need the 3P or second-pack decision | BMS telemetry reaches the Pi over UART |

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
| XT90S, XT60, 8 AWG + 22 AWG silicone wire | $37 | |
| XT60 charge pigtail parts | $5 | |
| Printed case (PC FR or ASA) | $15 | filament |
| Handheld spot welder | $40 | Joey's pick |
| **Total, pack** | **about $480** | |
| *On the power board budget (Joey):* JBD SP17S005 60 A + USB-UART lead about $50, trimmable 4 A charger $50 | *about $100* | |

Optional later: a 150 W electronic load ($40) for a controlled capacity test (otherwise the board's coulomb counter measures
capacity); a stronger welder for a copper-busbar leg pack; a cell IR meter ($35) if the multimeter can't grade cells (a DC
meter can't read 6 mΩ, so grading falls back to matching resting voltage plus the vendor's batch).

## 10. Pack sense board

Dropped 2026-11-06 22:44Z with the move to an on-pack BMS. The BMS's own balance harness connects straight to the taps.

## 11. Open items

| Item | Owner | Status |
| --- | --- | --- |
| JBD SP17S005 60 A, Li-ion: confirm the seller (section 3.1), buy a USB-UART lead, check silkscreen, 13-cell reading and body-diode behaviour on the unit; read the Endless-Sphere thread first | this thread | when the pack is bought |
| Measured pack DCIR | this thread | after build step 6 |
| Measured per-arm bus current | arm trunk thread | after the bench shoulder runs |
| Legs: 13S3P (39 cells, about 3.3 kg) or a pelvis pack in parallel | this thread + frame | later, when the leg loads are known |
| D2 door grown to pass a 95 × 162 pack | body shell thread | already listed |
| Pack case CAD (printed, rails, pull tab, lid facing the board at Z 34 with 0.5 mm Nomex) | this thread, on Joey-PC | next, after cells arrive and the plate layout is drawn |
