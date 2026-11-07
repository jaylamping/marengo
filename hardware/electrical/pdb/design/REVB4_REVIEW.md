# Power board rev B4: schematic review, round 4

2026-11-07. The packet was built from main at 43fbb6ab (58 hashed files). Every run used High effort with a 45 minute limit on Joey-PC. The open question was "ready for layout, apart from the bench measurements?". **Result: NO-GO from all six reviewers.** The E-stop topology, the latch, the interlock logic, the J24 pinout and the 10b option's polarity all held. What failed:
- the absorber margin, which rested on optimistic corners;
- the protection around U2's feed;
- a few unproven ratings.

No reviewer asked for a new block.

| Reviewer | Tool | Effort | Minutes | Verdict | On-target check |
| --- | --- | --- | --- | --- | --- |
| Fable | Claude CLI, API | high | 32.7 | NO-GO | Pass. $14.47 |
| Opus | agy, then API | high | 24.8 | NO-GO | agy quota was out ("resets in 37m"), so it ran on the API per the routing order. Pass. $4.80 |
| Sol | Codex | high | 29.8 | NO-GO | Pass |
| Astra | Codex | high | 18.4 | NO-GO | Pass |
| Muse (Wonder) | Muse | high | 18.8 | NO-GO | Pass |
| DeepSeek V4.1 Flash (trial) | OpenCode review agent, CheaperInference | high | 6.5 | NO-GO | **Fail on values**: names 146 of 147, but only 42 of 72 cited values match the netlist. It quoted a "marengo_pdb" symbol library that isn't in the packet. $0.07 |

Hashes matched before and after every run.

## Accepted and fixed (fix round 4)

| # | Finding | Raised by | How verified | Fix |
| --- | --- | --- | --- | --- |
| 1 | **The absorber margin rests on optimistic corners.** absorber_sim used a 0.1 Ω loop, D1 at its minimum breakdown, nominal C38, and a trip time that starts at the short rather than at the threshold crossing. Cell AC resistance (6.5 mΩ vs 12.8 mΩ DC) puts the pulse loop nearer 0.05 Ω. | Fable (blocker), Sol, Astra (blockers), Opus, Muse | Reran absorber_sim with corners: 0.05 Ω, D1 71.2 V, C −20 %, trip 8.8–17.5 µs. One can: **98–118 V**. Two: 86–100 V. Three: **80–91 V** | Three APZ1012470M100R cans fitted (C38, C44, C45). The corner table is now in absorber_sim.py. Bring-up step 1 measures pulse R, not DC R |
| 2 | "A makes the hard edge" is only true for an E-stop. In a bus short, U1's breaker can open B first, so the spike lands on MID with A on. U17's VBUS pin is rated 85 V, and R47 limits current but doesn't clamp. | Fable, Sol, Astra | Timing: U1 CB 0.44–1.2 µs plus gate discharge, against U8 4.3 µs plus threshold crossing | Note withdrawn and rewritten. A 68 V zener (D18) sits at U17's pin behind R47. The three cans bound BAT, and so MID |
| 3 | **F1 can't be shown to interrupt.** The 0466 is 125 V / 50 A breaking, and a short at U2's input sees 70 A or more. C39's inrush at every BMS close is near F1's melting I²t. D8 (BAV19W, about 0.7 V) doesn't hold U2's 0.3 V OUT–IN limit. SMAJ15A clamps up to 24.4 V, above Q6's 20 V gate rating. | Sol (blocker), Astra (blocker), Opus (major), Fable, Muse | Arithmetic: 54.6 / (0.69 + 0.1) Ω; SMAJ15A table | BAT → F1 → **22 Ω 2512 pulse-rated (R59)** → **series ES1D (D8 moved)** → BAT_CTL. A shorted U2 now draws about 1.6 A, so D9 clamps at 19.2 V or less and F1 opens far inside its rating. C39 inrush is about 2.5 A. With the series diode, C5 can't back-drive U2. F1's value string is now 125 V |
| 4 | FLT reaches UVLO through a plain resistor, so a failed U8 driving FLT high could enable B independently of RUN. | Sol | Netlist: R52 FLT_A–UVLO | BAT46W (D17) in series, pull-down only. UVLO with FLT low is about 1.9 V (was 1.7), still under 2.45 V. D12 15 V → 12 V, which keeps FLT inside its 15 V recommended range with zener tolerance |
| 5 | U7's timer may not cover the DVE's unpublished input capacitance at the low power-limit corner. | Muse (blocker), Astra | 45 W × 3.3 ms leaves under 47 µF of headroom | C19 100 → 220 nF (7–17 ms). Q7 at about 60 W for 17 ms is far inside its SOA. Bring-up step 6 scopes it |
| 6 | A J24 pin 5–11 short puts +12 V straight onto Q12. | Sol | Pinout | 220 Ω (R60) in Q12's drain: 0.45 V at 2 mA (ON needs 0.8 V or less), 55 mA into a short |
| 7 | No RC placeholder across U8's CS± (TI §8: nuisance SCP trips at high di/dt). A nuisance trip latches A, and FLT drops B. | Fable, Opus | tps4810 text p26 | CS+/CS− through 0R (R61/R62), 1 nF C0G DNP (C46) |
| 8 | eFuse outputs: TPS2595 OUT is rated −1.2 V for under 1 µs. U18 has nothing, and D5 is a TVS, not a Schottky. | Astra, Opus, Fable | tps2595 abs max | D5 → B340A Schottky. Same part (D19) on SPARE_ON |
| 9 | INA228 straps on SDA need extra SDA hold time (INA228 §7.5.1). | Opus | Datasheet; the three meters on SDA were waist, 12 V and MID | Re-strapped onto the nine GND/VS/SCL combinations. **New addresses: waist 0x4C, 12 V 0x4D, MID 0x4F** (software requirement) |
| 10 | Jetson, Pi buck and fan outputs are unfused, behind a 25 A converter that limits at about 30 A. | DeepSeek | Netlist: J5–J7 straight on +12V | 0466 fuses: 5 A, 5 A and 2 A (F3–F5) |
| 11 | VBUS bleeds only through U1's GATE sink. With drive caps, reset can stay blocked for minutes. | Fable, Opus, Muse | 2 mA into 10 mF | DNP 2512 bleeder footprint (R63). Bring-up step 5 times the bleed |
| 12 | Small value and package fixes. | Opus, Astra, Sol | Arithmetic | R6 0402 → 0805 (90 mW in the bridge case). C5 1206 → 1210 (DC-bias derating). R40 487 → 499 Ω (the −1 % corner stays at TI's minimum or above). R54 spec'd as pulse-rated |
| 13 | Stale or wrong sheet claims: "100 V diode" FLT note, the 33 µF converter cap, the ISCP spread (158–242 A, not 134–313), MID 0.48–0.50 and 63 mW, VBUS threshold "about 6 V" (really 6–8.5 V with hFE spread), self-test 0.4–0.6. | Fable, Muse, Opus, Sol, Astra | Sheet against netlist | Notes rewritten |
| 14 | The self-test window misses hot-leakage corners: 200 µA per bank moves MID to 0.34–0.59 × pack. | Sol, Astra | Arithmetic with U17's 0.8–1.2 MΩ | Window is now 0.33–0.60 × pack (REV_B_PARTS) |
| 15 | SPARE_SW's 4 A budget equals U9's minimum limit. | Opus | Budget table | Servo cap 3 A in the budget and firmware |
| 16 | Software and interface requirements: GPIO with no pull on the pressed input; show VBUS from the highest limb meter (U11 reads after the L-arm fuse); bus peaks under 90 A (U1 latches at 97 A and over); drive bus capacitance at most 20 mF; buy the 12 V STS3215. | Fable, Opus, Muse, Astra | | REV_B_PARTS |

ERC is clean and check.py passes 91 of 91; it now also checks the series feed, the FLT diode, the three cans, the U17 clamp, the straps, the fuses and the output clamps. latch_sim passes 54 of 54. The sheet went from A1 to A0 for the new block 10c. 200 parts.

## For Joey

1. **Braking resistor energy.** Sol and Astra (blockers), Muse, Opus and Fable all say the same: HS50 is 50 W on Arcol's heatsink, and the 5 A software cap lets about 284 W in. The "32 g × aluminium = 1.4 kJ" argument isn't a rating; hotspot temperature and pulse capability govern. The thermostat has no part number or setpoint. This doesn't change the board, but the acceptance needs one of:
   - (a) a software energy cap: about 1 kJ per burst at about 15 % duty, and a named thermostat;
   - (b) a bigger resistor (Arcol HS150/HS200, or two HS50 in series-parallel on the gusset).
   **My recommendation: (b) with a named NC thermostat**, because regen energy isn't something the board can see.
2. **Decided, Joey 2026-11-07: (b), done.** SMBJ15A (D20) on +12V, plus bring-up step 7 (back-drive scope test). **Servo back-feed (rev B3 M8 acceptance).** Sol and Astra say "C41 and the compute loads take it" isn't bounded: about 485 µF holds only 44 mJ from 12 to 18 V, and the compute load can be zero during shutdown. Options:
   - (a) a bench measurement: back-drive the neck at speed and scope +12V;
   - (b) a 15 V TVS on +12V, one part.
   **My recommendation: (b)**, plus (a) at bring-up.
3. **Decided, Joey 2026-11-07: stays DNP, gated on the JK bench test.** **10b fitted by default?** Fable says fit it, because a JK trip on close is a 60 s lockout. Muse and Opus say keep it DNP until the bench test. With three cans the close surge is about 300 A. I've left it DNP and gated on the bench test, because Q13's µs pulse rating (it would then carry the trip current) isn't confirmed.

## Before layout starts (not schematic)

- **Footprint audit with frozen MPNs.** Raised by Muse (blocker), Opus, Fable and Astra. CSS4J shunts on the Isabellenhütte BVR land, Q3 MMBTA92 vendor pinout, J20/J24 mating drawings, SK810L, BSC040, 0466 and 0997 holders. This is the first layout task.
- **Thermal copper.** About 2.6 W per main FET at 60 A, and the 40 K/W datasheet example gives about 100 K rise (Astra, Sol). Copper and airflow are layout requirements.
- **Floorplan.** Three cans and about 15 small parts are added; the floorplan needs a refresh.

## Rejected

| Claim | By | Why |
| --- | --- | --- |
| U1 power limit set to about 1.7 kW, so it never reaches the breaker; change RPWR to 14k | DeepSeek (blocker) | Formula invented. Sol and Astra both derive 652 W typical from the datasheet's VDS-dependent term. This also closes the 544 vs 652 W dispute: 544 W omitted that term (Muse still quotes 544) |
| FLT low doesn't hold UVLO low | DeepSeek (major) | Used RUN = 12 V. RUN is about 7 V (latch_sim, and every other reviewer) |
| Clamp fires on a healthy pack and on every E-stop | DeepSeek | 56.8 V is above 54.6 V; the clamp watches VBUS, behind the banks |
| Delete D15 | DeepSeek | Without it the 2N7002 gate sees 54 V through 470k |
| INA228 VBUS rated 36–48 V; U17 reads 0 | DeepSeek | INA228 VBUS is 0–85 V |
| 10b starts with the FET on | DeepSeek | Q13 is off at close, so the surge goes through R54. Every other reviewer checked it as right |
| J24 rows are 1,3,5…; TPS48100 DGX has 20 pads | DeepSeek | Fable, Opus, Sol and Astra checked the stock footprint (pin n faces n+6). DGX0019A has 19 leads |
| Trip timing in the sim is conservative | Muse | Overturned by the threshold-crossing time (Sol, Astra, Opus) |
| R30 is redundant | Fable | Kept: it holds bank A's gate off during U8's own UVLO window. One 0402 part |
| R46 → 1 MΩ | Opus | 0.55 mA is negligible beside the Pi; a faster bleed is worth more |
| D4 3 A rating is exceeded | DeepSeek | D4 only freewheels the lead inductance for microseconds |

## Usage

- Fable: $14.47 (API JSON).
- Opus: $4.80 (API; the agy quota was out).
- DeepSeek: $0.07 (CheaperInference).
- Codex and Muse: subscriptions.

## My errors

- **The absorber "81–83 V" was a nominal result presented as a bound.** I left out D1's tolerance, C38's tolerance, the threshold-crossing time and the cell AC resistance.
- **D8 as a parallel reverse diode never met U2's 0.3 V limit.** Sol flagged U2's reverse path in rev B2; I picked a silicon diode for it.
- **I didn't check F1's breaking rating.** I also carried "63 V" from the wrong series.
